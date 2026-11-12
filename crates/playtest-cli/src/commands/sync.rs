//! `playtest sync` and `playtest sync --check` implementation (`Spec 0013 §§3-5, 8`).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use playtest_core::{
    DiscoveredWorkspace, WORKSPACE_DIR_NAME, WORKSPACE_GITIGNORE_CONTENTS, WORKSPACE_GITIGNORE_FILE,
};

use crate::commands::init::normalize_mirror_language;
use crate::mirrors::{RenderedMirror, expected_mirror_filename, render_mirror, wiring_command_for};
use crate::output::{
    JSON_SCHEMA_VERSION, MirrorDriftEntry, MirrorDriftKind, MirrorSyncStatus, SyncCheckReport,
    SyncReport, SyncedMirrorEntry,
};

const SKIPPED_WALK_DIRS: &[&str] = &[
    ".playtest",
    ".git",
    ".pixi",
    ".pixi-sandbox",
    "node_modules",
    "target",
    ".venv",
    "venv",
    "dist",
    "build",
    "__pycache__",
];

fn unique_configured_languages(raw_languages: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(raw_languages.len());
    for raw in raw_languages {
        let canonical = normalize_mirror_language(raw).map_err(|_| {
            format!(
                "unsupported mirror language `{raw}` in `playtest.toml` (supported in v0.1: `python`, `typescript`)"
            )
        })?;
        if !out.iter().any(|existing| existing == canonical) {
            out.push(canonical.to_owned());
        }
    }
    Ok(out)
}

fn render_configured_mirrors(
    languages: &[String],
    cli_version: &str,
) -> Result<Vec<RenderedMirror>, String> {
    languages
        .iter()
        .map(|lang| render_mirror(lang, cli_version))
        .collect()
}

fn remove_fs_entry(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path)
        .map_err(|err| format!("failed to inspect {}: {err}", path.display()))?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        fs::remove_dir_all(path)
            .map_err(|err| format!("failed to remove stale directory {}: {err}", path.display()))
    } else {
        fs::remove_file(path)
            .map_err(|err| format!("failed to remove stale file {}: {err}", path.display()))
    }
}

fn relative_posix(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|comp| comp.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Synchronize `.playtest/<lang>/` mirrors deterministically from embedded SDK assets (`Spec 0013 §3`).
pub(crate) fn execute_sync(
    workspace: &DiscoveredWorkspace,
    cli_version: &str,
) -> Result<SyncReport, String> {
    let languages = unique_configured_languages(&workspace.config.mirrors.languages)?;
    let rendered_mirrors = render_configured_mirrors(&languages, cli_version)?;
    let configured_set: BTreeSet<&str> = languages.iter().map(String::as_str).collect();

    let playtest_dir = workspace.root.join(WORKSPACE_DIR_NAME);
    fs::create_dir_all(&playtest_dir)
        .map_err(|err| format!("failed to create {}: {err}", playtest_dir.display()))?;

    let gitignore_path = playtest_dir.join(WORKSPACE_GITIGNORE_FILE);
    let write_gitignore = match fs::read_to_string(&gitignore_path) {
        Ok(existing) => existing != WORKSPACE_GITIGNORE_CONTENTS,
        Err(_) => true,
    };
    if write_gitignore {
        fs::write(&gitignore_path, WORKSPACE_GITIGNORE_CONTENTS)
            .map_err(|err| format!("failed to write {}: {err}", gitignore_path.display()))?;
    }

    let mut removed_files = Vec::new();
    let top_entries = fs::read_dir(&playtest_dir)
        .map_err(|err| format!("failed to read {}: {err}", playtest_dir.display()))?;
    for entry_res in top_entries {
        let entry = entry_res.map_err(|err| err.to_string())?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if name_str == WORKSPACE_GITIGNORE_FILE || configured_set.contains(name_str.as_ref()) {
            continue;
        }
        let stale_path = entry.path();
        removed_files.push(relative_posix(&workspace.root, &stale_path));
        remove_fs_entry(&stale_path)?;
    }

    let mut synced_entries = Vec::with_capacity(rendered_mirrors.len());
    let mut wiring_commands = Vec::new();

    for rendered in rendered_mirrors {
        let lang_dir = playtest_dir.join(rendered.language);
        if lang_dir.exists() && !lang_dir.is_dir() {
            removed_files.push(relative_posix(&workspace.root, &lang_dir));
            remove_fs_entry(&lang_dir)?;
        }
        fs::create_dir_all(&lang_dir)
            .map_err(|err| format!("failed to create {}: {err}", lang_dir.display()))?;

        let target_path = lang_dir.join(&rendered.filename);
        let existed_before = target_path.is_file();

        let lang_entries = fs::read_dir(&lang_dir)
            .map_err(|err| format!("failed to read {}: {err}", lang_dir.display()))?;
        for entry_res in lang_entries {
            let entry = entry_res.map_err(|err| err.to_string())?;
            if entry.file_name().to_string_lossy() == rendered.filename && entry.path().is_file() {
                continue;
            }
            let stale_path = entry.path();
            removed_files.push(relative_posix(&workspace.root, &stale_path));
            remove_fs_entry(&stale_path)?;
        }

        let status = if !existed_before {
            fs::write(&target_path, &rendered.bytes)
                .map_err(|err| format!("failed to write {}: {err}", target_path.display()))?;
            MirrorSyncStatus::Created
        } else {
            let current = fs::read(&target_path)
                .map_err(|err| format!("failed to read {}: {err}", target_path.display()))?;
            if current == rendered.bytes {
                MirrorSyncStatus::Unchanged
            } else {
                fs::write(&target_path, &rendered.bytes)
                    .map_err(|err| format!("failed to write {}: {err}", target_path.display()))?;
                MirrorSyncStatus::Updated
            }
        };

        let rel_path = format!(
            "{WORKSPACE_DIR_NAME}/{}/{}",
            rendered.language, rendered.filename
        );
        let path_changed = !existed_before;
        if path_changed && let Some(cmd) = wiring_command_for(rendered.language, &rel_path) {
            wiring_commands.push(cmd);
        }

        synced_entries.push(SyncedMirrorEntry {
            language: rendered.language.to_owned(),
            relative_path: rel_path,
            artifact_path: target_path,
            sha256: rendered.sha256,
            size_bytes: rendered.bytes.len(),
            status,
            path_changed,
        });
    }

    removed_files.sort();
    Ok(SyncReport {
        schema_version: JSON_SCHEMA_VERSION,
        workspace_root: workspace.root.clone(),
        version: cli_version.to_owned(),
        mirrors: synced_entries,
        removed_files,
        wiring_commands,
    })
}

/// Extract `.playtest/<lang>/<artifact>` references from a consumer `pyproject.toml` or `package.json`.
fn extract_playtest_mirror_refs(contents: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let needle = ".playtest/";
    let mut search_from = 0;
    while let Some(offset) = contents[search_from..].find(needle) {
        let start = search_from + offset;
        let tail = &contents[start + needle.len()..];
        let end_rel = tail
            .find(|ch: char| {
                ch.is_whitespace()
                    || matches!(ch, '"' | '\'' | ',' | ')' | ']' | '}' | '#' | ';' | '>')
            })
            .unwrap_or(tail.len());
        let token = &tail[..end_rel];
        if let Some((lang, artifact)) = token.split_once('/')
            && !lang.is_empty()
            && !artifact.is_empty()
            && !artifact.contains('/')
        {
            let pair = (lang.to_owned(), artifact.to_owned());
            if !found.contains(&pair) {
                found.push(pair);
            }
        }
        search_from = start + needle.len() + end_rel;
    }
    found
}

fn scan_consumer_manifests_for_stale_wiring(
    workspace_root: &Path,
    dir: &Path,
    cli_version: &str,
    issues: &mut Vec<MirrorDriftEntry>,
    wiring_fixes: &mut Vec<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|err| {
        format!(
            "failed to scan workspace directory {}: {err}",
            dir.display()
        )
    })?;
    let mut sorted_paths = Vec::new();
    for entry_res in entries {
        let entry = entry_res.map_err(|err| err.to_string())?;
        sorted_paths.push(entry.path());
    }
    sorted_paths.sort();

    for path in sorted_paths {
        let Some(name) = path.file_name().and_then(|os| os.to_str()) else {
            continue;
        };
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if SKIPPED_WALK_DIRS.contains(&name) {
                continue;
            }
            scan_consumer_manifests_for_stale_wiring(
                workspace_root,
                &path,
                cli_version,
                issues,
                wiring_fixes,
            )?;
            continue;
        }
        if name != "pyproject.toml" && name != "package.json" {
            continue;
        }
        let text = fs::read_to_string(&path)
            .map_err(|err| format!("failed to read consumer manifest {}: {err}", path.display()))?;
        let manifest_rel = relative_posix(workspace_root, &path);
        for (lang, artifact) in extract_playtest_mirror_refs(&text) {
            let referenced_rel = format!("{WORKSPACE_DIR_NAME}/{lang}/{artifact}");
            let referenced_abs = workspace_root
                .join(WORKSPACE_DIR_NAME)
                .join(&lang)
                .join(&artifact);
            let expected_name = expected_mirror_filename(&lang, cli_version).ok();
            let matches_current = expected_name.as_deref() == Some(artifact.as_str());
            if !referenced_abs.is_file() || !matches_current {
                let detail = match expected_name {
                    Some(ref current_file) if current_file != &artifact => format!(
                        "references outdated mirror `{referenced_rel}` (current mirror for {cli_version} is `{WORKSPACE_DIR_NAME}/{lang}/{current_file}`)"
                    ),
                    _ => format!("references missing mirror `{referenced_rel}`"),
                };
                issues.push(MirrorDriftEntry {
                    kind: MirrorDriftKind::StaleWiring,
                    path: manifest_rel.clone(),
                    detail,
                });
                if let Some(ref current_file) = expected_name {
                    let current_rel = format!("{WORKSPACE_DIR_NAME}/{lang}/{current_file}");
                    if let Some(cmd) = wiring_command_for(&lang, &current_rel)
                        && !wiring_fixes.contains(&cmd)
                    {
                        wiring_fixes.push(cmd);
                    }
                }
            }
        }
    }
    Ok(())
}

/// Verify `.playtest/` mirrors and consumer manifest wiring without writing anything (`Spec 0013 §§3.5, 8`).
pub(crate) fn execute_sync_check(
    workspace: &DiscoveredWorkspace,
    cli_version: &str,
) -> Result<SyncCheckReport, String> {
    let languages = unique_configured_languages(&workspace.config.mirrors.languages)?;
    let rendered_mirrors = render_configured_mirrors(&languages, cli_version)?;
    let configured_set: BTreeSet<&str> = languages.iter().map(String::as_str).collect();

    let mut issues = Vec::new();
    let playtest_dir = workspace.root.join(WORKSPACE_DIR_NAME);
    let gitignore_path = playtest_dir.join(WORKSPACE_GITIGNORE_FILE);
    let gitignore_rel = format!("{WORKSPACE_DIR_NAME}/{WORKSPACE_GITIGNORE_FILE}");

    if !gitignore_path.is_file() {
        issues.push(MirrorDriftEntry {
            kind: MirrorDriftKind::Missing,
            path: gitignore_rel,
            detail: "expected `.playtest/.gitignore` is missing".to_owned(),
        });
    } else {
        let existing = fs::read_to_string(&gitignore_path)
            .map_err(|err| format!("failed to read {}: {err}", gitignore_path.display()))?;
        if existing != WORKSPACE_GITIGNORE_CONTENTS {
            issues.push(MirrorDriftEntry {
                kind: MirrorDriftKind::Modified,
                path: gitignore_rel,
                detail: "`.playtest/.gitignore` differs from expected Playtest contents".to_owned(),
            });
        }
    }

    if playtest_dir.is_dir() {
        let top_entries = fs::read_dir(&playtest_dir)
            .map_err(|err| format!("failed to read {}: {err}", playtest_dir.display()))?;
        for entry_res in top_entries {
            let entry = entry_res.map_err(|err| err.to_string())?;
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str == WORKSPACE_GITIGNORE_FILE || configured_set.contains(name_str.as_ref()) {
                continue;
            }
            issues.push(MirrorDriftEntry {
                kind: MirrorDriftKind::Stale,
                path: relative_posix(&workspace.root, &entry.path()),
                detail: "unconfigured or stale entry in `.playtest/`".to_owned(),
            });
        }
    }

    for rendered in &rendered_mirrors {
        let lang_dir = playtest_dir.join(rendered.language);
        let expected_rel = format!(
            "{WORKSPACE_DIR_NAME}/{}/{}",
            rendered.language, rendered.filename
        );
        let target_path = lang_dir.join(&rendered.filename);

        if !target_path.is_file() {
            issues.push(MirrorDriftEntry {
                kind: MirrorDriftKind::Missing,
                path: expected_rel,
                detail: format!(
                    "expected {} SDK mirror for playtest {} is missing",
                    rendered.language, cli_version
                ),
            });
        } else {
            let actual = fs::read(&target_path)
                .map_err(|err| format!("failed to read {}: {err}", target_path.display()))?;
            if actual != rendered.bytes {
                issues.push(MirrorDriftEntry {
                    kind: MirrorDriftKind::Modified,
                    path: expected_rel,
                    detail: format!(
                        "{} SDK mirror bytes differ from embedded playtest {} assets",
                        rendered.language, cli_version
                    ),
                });
            }
        }

        if lang_dir.is_dir() {
            let lang_entries = fs::read_dir(&lang_dir)
                .map_err(|err| format!("failed to read {}: {err}", lang_dir.display()))?;
            for entry_res in lang_entries {
                let entry = entry_res.map_err(|err| err.to_string())?;
                if entry.file_name().to_string_lossy() == rendered.filename
                    && entry.path().is_file()
                {
                    continue;
                }
                issues.push(MirrorDriftEntry {
                    kind: MirrorDriftKind::Stale,
                    path: relative_posix(&workspace.root, &entry.path()),
                    detail: format!("stale file in `.playtest/{}/`", rendered.language),
                });
            }
        }
    }

    let mut wiring_fixes = Vec::new();
    scan_consumer_manifests_for_stale_wiring(
        &workspace.root,
        &workspace.root,
        cli_version,
        &mut issues,
        &mut wiring_fixes,
    )?;

    issues.sort();

    let mut fixes = Vec::new();
    if issues
        .iter()
        .any(|item| item.kind != MirrorDriftKind::StaleWiring)
    {
        fixes.push("playtest sync".to_owned());
    }
    fixes.extend(wiring_fixes);

    Ok(SyncCheckReport {
        schema_version: JSON_SCHEMA_VERSION,
        workspace_root: workspace.root.clone(),
        version: cli_version.to_owned(),
        ok: issues.is_empty(),
        languages,
        issues,
        fixes,
    })
}
