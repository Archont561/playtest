//! `playtest init` implementation (`Spec 0013 §§1, 2, 5`).

use std::fs;
use std::io;
use std::path::Path;

use playtest_core::{
    DiscoveredWorkspace, WORKSPACE_DIR_NAME, WORKSPACE_GITIGNORE_CONTENTS,
    WORKSPACE_GITIGNORE_FILE, WORKSPACE_MANIFEST_FILE, WorkspaceConfig,
};

use crate::RunEnvironment;
use crate::output::{InitReport, JSON_SCHEMA_VERSION};

/// Supported mirror language identifiers accepted by `playtest init --lang` in Milestone M1 (`Spec 0013 §4`).
pub(crate) fn normalize_mirror_language(raw: &str) -> Result<&'static str, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "python" | "py" => Ok("python"),
        "typescript" | "ts" => Ok("typescript"),
        other => Err(format!(
            "unsupported mirror language `{other}` for `playtest init` (supported in v0.1: `python`, `typescript`)"
        )),
    }
}

fn build_next_steps(version: &str, languages: &[String]) -> Vec<String> {
    let mut steps = vec!["playtest sync".to_owned()];
    for lang in languages {
        match lang.as_str() {
            "python" => steps.push(format!(
                "uv add --dev .playtest/python/playtest_py-{version}-py3-none-any.whl"
            )),
            "typescript" => steps.push(format!(
                "npm add --save-dev .playtest/typescript/archont561-playtest-{version}.tgz  # or pnpm add --save-dev / bun add --dev"
            )),
            _ => {}
        }
    }
    steps
        .push("cargo add playtest --dev  # for Rust crates (uses crates.io, no mirror)".to_owned());
    steps
}

/// Execute `playtest init` idempotently (`Spec 0013 §2`).
///
/// - Never overwrites an existing `playtest.toml` or `.playtest/.gitignore`.
/// - Never creates or edits any consumer manifest or lockfile.
pub(crate) fn execute_init(
    env: &RunEnvironment,
    existing_workspace: Option<&DiscoveredWorkspace>,
    requested_languages: Option<&[String]>,
) -> io::Result<InitReport> {
    let workspace_root = existing_workspace.map_or_else(|| env.cwd.clone(), |ws| ws.root.clone());
    let manifest_path = workspace_root.join(WORKSPACE_MANIFEST_FILE);
    let playtest_dir = workspace_root.join(WORKSPACE_DIR_NAME);
    let gitignore_path = playtest_dir.join(WORKSPACE_GITIGNORE_FILE);

    let (config, created_manifest) = if let Some(existing) = existing_workspace {
        (existing.config.clone(), false)
    } else {
        let cfg = match requested_languages {
            Some(langs) => WorkspaceConfig::new(env.cli_version.clone(), langs.to_vec()),
            None => WorkspaceConfig::default_for_version(env.cli_version.clone()),
        };
        write_new_file(&manifest_path, &cfg.to_toml_string())?;
        (cfg, true)
    };

    let created_gitignore = if gitignore_path.exists() {
        false
    } else {
        fs::create_dir_all(&playtest_dir)?;
        write_new_file(&gitignore_path, WORKSPACE_GITIGNORE_CONTENTS)?;
        true
    };

    let next_steps = build_next_steps(&config.version, &config.mirrors.languages);

    Ok(InitReport {
        schema_version: JSON_SCHEMA_VERSION,
        workspace_root,
        manifest_path,
        gitignore_path,
        version: config.version,
        languages: config.mirrors.languages,
        created_manifest,
        created_gitignore,
        next_steps,
    })
}

fn write_new_file(path: &Path, contents: &str) -> io::Result<()> {
    fs::write(path, contents)
}
