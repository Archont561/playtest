//! Build script for `playtest-assets` (`Spec 0012 §2`).
//!
//! Copies the explicit published SDK file list from `<workspace_root>/<lang>/` (or from
//! `CARGO_MANIFEST_DIR/staged/<lang>/` when building from a packaged `.crate` archive)
//! into `$OUT_DIR/embedded_sdks/<lang>/` for compile-time embedding via `include_dir`.
//! Rejects symbolic links on every inspected path component and creates none.

use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct AssetsManifest {
    languages: BTreeMap<String, LanguageSpec>,
}

#[derive(Debug, Deserialize)]
struct LanguageSpec {
    source_dir: String,
    #[serde(default)]
    generated: Vec<String>,
    files: Vec<String>,
}

fn emit_cargo_directive(line: &str) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{line}")
}

fn ensure_no_symlink_components(base: &Path, rel: &Path) -> Result<(), String> {
    if fs::symlink_metadata(base)
        .map_err(|err| format!("failed to inspect {}: {err}", base.display()))?
        .file_type()
        .is_symlink()
    {
        return Err(format!(
            "symbolic link is forbidden at root {}",
            base.display()
        ));
    }

    let mut cursor = base.to_path_buf();
    for component in rel.components() {
        match component {
            Component::Normal(part) => {
                cursor.push(part);
                let meta = fs::symlink_metadata(&cursor).map_err(|err| {
                    format!(
                        "missing or unreadable listed SDK asset {} (under {}): {err}",
                        rel.display(),
                        base.display()
                    )
                })?;
                if meta.file_type().is_symlink() {
                    return Err(format!(
                        "symbolic link is forbidden in embedded SDK path: {}",
                        cursor.display()
                    ));
                }
            }
            _ => {
                return Err(format!(
                    "non-normal path component in assets_manifest.toml: {}",
                    rel.display()
                ));
            }
        }
    }
    Ok(())
}

fn resolve_language_root(
    manifest_dir: &Path,
    use_staged: bool,
    lang: &str,
    spec: &LanguageSpec,
) -> Result<PathBuf, String> {
    if use_staged {
        return Ok(manifest_dir.join("staged").join(lang));
    }
    let workspace_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| {
            format!(
                "cannot derive workspace root from CARGO_MANIFEST_DIR={} without staged/ directory",
                manifest_dir.display()
            )
        })?;
    Ok(workspace_root.join(&spec.source_dir))
}

fn normalize_asset_bytes(lang: &str, rel_str: &str, raw: Vec<u8>) -> Vec<u8> {
    let Ok(text) = std::str::from_utf8(&raw) else {
        return raw;
    };
    let lf = text.replace("\r\n", "\n");
    if lang != "typescript" || !rel_str.starts_with("dist/") {
        return lf.into_bytes();
    }
    let mut out = String::with_capacity(lf.len());
    for chunk in lf.split_inclusive('\n') {
        if let Some(rest) = chunk.strip_prefix("// ") {
            out.push_str("// ");
            out.push_str(&rest.replace('\\', "/"));
        } else {
            out.push_str(chunk);
        }
    }
    out.into_bytes()
}

fn copy_language_files(
    lang: &str,
    spec: &LanguageSpec,
    lang_root: &Path,
    out_root: &Path,
) -> Result<(), Box<dyn Error>> {
    if !lang_root.exists() {
        return Err(format!(
            "SDK source directory for `{lang}` does not exist at {}",
            lang_root.display()
        )
        .into());
    }
    for rel_str in &spec.files {
        let rel_path = Path::new(rel_str);
        if let Err(reason) = ensure_no_symlink_components(lang_root, rel_path) {
            let hint = if spec.generated.iter().any(|item| item == rel_str) {
                " (generated build artifact: run `pixi run build-ts-dist` before compiling `playtest-assets`)"
            } else {
                ""
            };
            return Err(format!("{reason}{hint}").into());
        }
        let src_file = lang_root.join(rel_path);
        let meta = fs::symlink_metadata(&src_file)?;
        if !meta.file_type().is_file() {
            return Err(format!(
                "listed SDK asset for `{lang}` is not a regular file: {}",
                src_file.display()
            )
            .into());
        }
        emit_cargo_directive(&format!("cargo:rerun-if-changed={}", src_file.display()))?;
        let dst_file = out_root.join(lang).join(rel_path);
        if let Some(parent) = dst_file.parent() {
            fs::create_dir_all(parent)?;
        }
        let raw_bytes = fs::read(&src_file)?;
        let normalized = normalize_asset_bytes(lang, rel_str, raw_bytes);
        fs::write(&dst_file, normalized)?;
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    let manifest_path = manifest_dir.join("assets_manifest.toml");
    emit_cargo_directive(&format!(
        "cargo:rerun-if-changed={}",
        manifest_path.display()
    ))?;
    let staged_dir = manifest_dir.join("staged");
    emit_cargo_directive(&format!("cargo:rerun-if-changed={}", staged_dir.display()))?;

    let raw_manifest = fs::read_to_string(&manifest_path)?;
    let manifest: AssetsManifest = toml::from_str(&raw_manifest)?;

    let use_staged = staged_dir.exists();
    if use_staged && fs::symlink_metadata(&staged_dir)?.file_type().is_symlink() {
        return Err(format!(
            "symbolic link is forbidden at staged root {}",
            staged_dir.display()
        )
        .into());
    }

    let embedded_root = out_dir.join("embedded_sdks");
    if embedded_root.exists() {
        fs::remove_dir_all(&embedded_root)?;
    }
    fs::create_dir_all(&embedded_root)?;

    for (lang, spec) in &manifest.languages {
        let lang_root = resolve_language_root(&manifest_dir, use_staged, lang, spec)?;
        copy_language_files(lang, spec, &lang_root, &embedded_root)?;
    }
    Ok(())
}
