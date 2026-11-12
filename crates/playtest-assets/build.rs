//! Build script for `playtest-assets` (`Spec 0012 §2`, `ADR 0020`).
//!
//! Reads the explicit published SDK file list from `<workspace_root>/<lang>/` (or from
//! `CARGO_MANIFEST_DIR/staged/<lang>/` when building from a packaged `.crate` archive),
//! normalizes text line endings for cross-OS determinism, packs all files in lexicographic
//! order into a deterministic POSIX `ustar` tar stream, and compresses that solid stream
//! with `brotli` (`quality = 11`, `lgwin = 22`, `portable-float`) into
//! `$OUT_DIR/embedded_sdks.tar.br` for compile-time `include_bytes!` embedding.
//! Rejects symbolic links on every inspected path component and creates none.

use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

const BROTLI_BUFFER_SIZE: usize = 4096;
const BROTLI_QUALITY: u32 = 11;
const BROTLI_LGWIN: u32 = 22;

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

fn append_language_entries(
    lang: &str,
    spec: &LanguageSpec,
    lang_root: &Path,
    tar_builder: &mut tar::Builder<Vec<u8>>,
) -> Result<(), Box<dyn Error>> {
    if !lang_root.exists() {
        return Err(format!(
            "SDK source directory for `{lang}` does not exist at {}",
            lang_root.display()
        )
        .into());
    }
    let mut sorted_files = spec.files.clone();
    sorted_files.sort();

    for rel_str in &sorted_files {
        let clean_rel = rel_str.replace('\\', "/");
        let rel_path = Path::new(&clean_rel);
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

        let raw_bytes = fs::read(&src_file)?;
        let normalized = normalize_asset_bytes(lang, &clean_rel, raw_bytes);
        let archive_path = format!("{lang}/{}", clean_rel.trim_start_matches('/'));

        let mut header = tar::Header::new_ustar();
        header.set_entry_type(tar::EntryType::Regular);
        header.set_size(normalized.len() as u64);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_username("")?;
        header.set_groupname("")?;
        header.set_device_major(0)?;
        header.set_device_minor(0)?;
        tar_builder.append_data(&mut header, &archive_path, normalized.as_slice())?;
    }
    Ok(())
}

fn compress_tar_brotli(tar_bytes: &[u8]) -> io::Result<Vec<u8>> {
    let mut writer =
        brotli::CompressorWriter::new(Vec::new(), BROTLI_BUFFER_SIZE, BROTLI_QUALITY, BROTLI_LGWIN);
    writer.write_all(tar_bytes)?;
    writer.flush()?;
    Ok(writer.into_inner())
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

    let mut tar_builder = tar::Builder::new(Vec::new());
    for (lang, spec) in &manifest.languages {
        let lang_root = resolve_language_root(&manifest_dir, use_staged, lang, spec)?;
        append_language_entries(lang, spec, &lang_root, &mut tar_builder)?;
    }
    let tar_bytes = tar_builder.into_inner()?;
    let compressed = compress_tar_brotli(&tar_bytes)?;

    let archive_out = out_dir.join("embedded_sdks.tar.br");
    fs::write(&archive_out, compressed)?;
    Ok(())
}
