//! Deterministic SDK mirror archive builders (`Spec 0013 §§3, 4`).
//!
//! Renders byte-identical installable SDK mirrors from `playtest-assets` across Linux,
//! macOS, and Windows:
//! - Python: `playtest_py-<version>-py3-none-any.whl` (PEP 427 `ZIP_STORED` archive)
//! - TypeScript: `archont561-playtest-<version>.tgz` (POSIX `ustar` + RFC 1952/1951 stored gzip)

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;

use playtest_assets::{SdkLanguage, embedded_files, get_embedded_file};
use serde_json::{Map, Value as JsonValue};
use sha2::{Digest, Sha256};
use tar::{Builder as TarBuilder, EntryType, Header as TarHeader};

/// Fixed POSIX `ustar` modification timestamp (`1985-10-26T08:15:00Z`, npm's reproducibility epoch).
const DETERMINISTIC_TAR_MTIME: u64 = 499_162_500;

/// Maximum payload bytes per RFC 1951 uncompressed (`BTYPE=00`) DEFLATE block.
const DEFLATE_STORED_BLOCK_MAX: usize = usize::MAX_VALUE_U16;

trait MaxU16 {
    const MAX_VALUE_U16: usize = 65_535;
}
impl MaxU16 for usize {}

/// Rendered deterministic SDK mirror artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedMirror {
    /// Canonical language identifier (`"python"` or `"typescript"`).
    pub language: &'static str,
    /// Ecosystem-conventional filename inside `.playtest/<language>/`.
    pub filename: String,
    /// Deterministic archive bytes.
    pub bytes: Vec<u8>,
    /// Lowercase hexadecimal SHA-256 digest of `bytes`.
    pub sha256: String,
}

/// Compute lowercase hex SHA-256 of `data`.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut out = String::with_capacity(64);
    for byte in digest {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

/// Encode `bytes` as URL-safe base64 without `=` padding (PEP 427 `RECORD` digest format).
#[must_use]
fn base64url_nopad(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity((bytes.len() * 4).div_ceil(3));
    let mut idx = 0;
    while idx + 3 <= bytes.len() {
        let triple = (u32::from(bytes[idx]) << 16)
            | (u32::from(bytes[idx + 1]) << 8)
            | u32::from(bytes[idx + 2]);
        out.push(char::from(ALPHABET[((triple >> 18) & 0x3f) as usize]));
        out.push(char::from(ALPHABET[((triple >> 12) & 0x3f) as usize]));
        out.push(char::from(ALPHABET[((triple >> 6) & 0x3f) as usize]));
        out.push(char::from(ALPHABET[(triple & 0x3f) as usize]));
        idx += 3;
    }
    let rem = bytes.len() - idx;
    if rem == 1 {
        let val = u32::from(bytes[idx]) << 16;
        out.push(char::from(ALPHABET[((val >> 18) & 0x3f) as usize]));
        out.push(char::from(ALPHABET[((val >> 12) & 0x3f) as usize]));
    } else if rem == 2 {
        let val = (u32::from(bytes[idx]) << 16) | (u32::from(bytes[idx + 1]) << 8);
        out.push(char::from(ALPHABET[((val >> 18) & 0x3f) as usize]));
        out.push(char::from(ALPHABET[((val >> 12) & 0x3f) as usize]));
        out.push(char::from(ALPHABET[((val >> 6) & 0x3f) as usize]));
    }
    out
}

/// Compute IEEE 802.3 CRC-32 (`0xEDB8_8320`) deterministically in safe pure Rust.
#[must_use]
fn crc32_ieee(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xffff_ffff;
    for &byte in data {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

/// Wrap `payload` in an RFC 1952 gzip stream using RFC 1951 `BTYPE=00` stored blocks.
///
/// Avoids platform- or SIMD-dependent LZ77 match differences in `zlib-rs` so `.tgz`
/// output is mathematically byte-identical across x86_64 and ARM64 on Linux, macOS, and Windows.
#[must_use]
fn deterministic_gzip(payload: &[u8]) -> Vec<u8> {
    let block_count = payload.len().div_ceil(DEFLATE_STORED_BLOCK_MAX).max(1);
    let mut out = Vec::with_capacity(18 + payload.len() + block_count * 5);
    // RFC 1952 header: ID1=0x1f, ID2=0x8b, CM=8 (deflate), FLG=0, MTIME=0, XFL=0, OS=255 (unknown).
    out.extend_from_slice(&[0x1f, 0x8b, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff]);

    if payload.is_empty() {
        out.extend_from_slice(&[0x01, 0x00, 0x00, 0xff, 0xff]);
    } else {
        let chunks: Vec<&[u8]> = payload.chunks(DEFLATE_STORED_BLOCK_MAX).collect();
        let last_index = chunks.len() - 1;
        for (idx, chunk) in chunks.into_iter().enumerate() {
            let bfinal = u8::from(idx == last_index);
            let len = chunk.len() as u16;
            let nlen = !len;
            out.push(bfinal);
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(&nlen.to_le_bytes());
            out.extend_from_slice(chunk);
        }
    }

    let crc = crc32_ieee(payload);
    let isize_mod = (payload.len() as u64 & 0xffff_ffff) as u32;
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&isize_mod.to_le_bytes());
    out
}

fn push_u16_le(buf: &mut Vec<u8>, val: u16) {
    buf.extend_from_slice(&val.to_le_bytes());
}

fn push_u32_le(buf: &mut Vec<u8>, val: u32) {
    buf.extend_from_slice(&val.to_le_bytes());
}

/// Write a deterministic PKWARE `ZIP_STORED` archive with fixed Unix (`system = 3`) headers.
///
/// The `zip` crate stamps `version_made_by` differently on `cfg(windows)` vs `cfg(unix)`;
/// emitting explicit little-endian PKWARE 2.0 headers guarantees identical `.whl` bytes across
/// Linux, macOS, and Windows.
#[must_use]
fn deterministic_stored_zip(entries: &BTreeMap<String, Vec<u8>>) -> Vec<u8> {
    const DOS_DATE_1980_01_01: u16 = 0x0021;
    const UTF8_NAME_FLAG: u16 = 0x0800;
    const VERSION_NEEDED_2_0: u16 = 20;
    const VERSION_MADE_BY_UNIX_2_0: u16 = 0x0314;
    const UNIX_REG_0644_ATTR: u32 = 0o100_644 << 16;

    let mut out = Vec::new();
    let mut central_dir = Vec::new();

    for (entry_path, data) in entries {
        let name_bytes = entry_path.as_bytes();
        let name_len = name_bytes.len() as u16;
        let data_len = data.len() as u32;
        let crc = crc32_ieee(data);
        let local_offset = out.len() as u32;

        push_u32_le(&mut out, 0x0403_4b50);
        push_u16_le(&mut out, VERSION_NEEDED_2_0);
        push_u16_le(&mut out, UTF8_NAME_FLAG);
        push_u16_le(&mut out, 0);
        push_u16_le(&mut out, 0);
        push_u16_le(&mut out, DOS_DATE_1980_01_01);
        push_u32_le(&mut out, crc);
        push_u32_le(&mut out, data_len);
        push_u32_le(&mut out, data_len);
        push_u16_le(&mut out, name_len);
        push_u16_le(&mut out, 0);
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);

        push_u32_le(&mut central_dir, 0x0201_4b50);
        push_u16_le(&mut central_dir, VERSION_MADE_BY_UNIX_2_0);
        push_u16_le(&mut central_dir, VERSION_NEEDED_2_0);
        push_u16_le(&mut central_dir, UTF8_NAME_FLAG);
        push_u16_le(&mut central_dir, 0);
        push_u16_le(&mut central_dir, 0);
        push_u16_le(&mut central_dir, DOS_DATE_1980_01_01);
        push_u32_le(&mut central_dir, crc);
        push_u32_le(&mut central_dir, data_len);
        push_u32_le(&mut central_dir, data_len);
        push_u16_le(&mut central_dir, name_len);
        push_u16_le(&mut central_dir, 0);
        push_u16_le(&mut central_dir, 0);
        push_u16_le(&mut central_dir, 0);
        push_u16_le(&mut central_dir, 0);
        push_u32_le(&mut central_dir, UNIX_REG_0644_ATTR);
        push_u32_le(&mut central_dir, local_offset);
        central_dir.extend_from_slice(name_bytes);
    }

    let cd_offset = out.len() as u32;
    let cd_size = central_dir.len() as u32;
    let count = entries.len() as u16;
    out.extend_from_slice(&central_dir);

    push_u32_le(&mut out, 0x0605_4b50);
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, 0);
    push_u16_le(&mut out, count);
    push_u16_le(&mut out, count);
    push_u32_le(&mut out, cd_size);
    push_u32_le(&mut out, cd_offset);
    push_u16_le(&mut out, 0);
    out
}

/// Return the canonical mirror filename for `language` and `version` (`Spec 0013 §4`).
pub fn expected_mirror_filename(language: &str, version: &str) -> Result<String, String> {
    let normalized_ver = normalize_wheel_version(version);
    match language {
        "python" => Ok(format!("playtest_py-{normalized_ver}-py3-none-any.whl")),
        "typescript" => Ok(format!("archont561-playtest-{version}.tgz")),
        other => Err(format!(
            "unsupported mirror language `{other}` in `playtest.toml` (supported in v0.1: `python`, `typescript`)"
        )),
    }
}

/// Return the canonical consumer wiring command for `language` and relative mirror path (`Spec 0013 §5`).
#[must_use]
pub fn wiring_command_for(language: &str, relative_mirror_path: &str) -> Option<String> {
    match language {
        "python" => Some(format!("uv add --dev {relative_mirror_path}")),
        "typescript" => Some(format!(
            "npm add --save-dev {relative_mirror_path}  # or pnpm add --save-dev @archont561/playtest@file:{relative_mirror_path} / bun add --dev @archont561/playtest@file:{relative_mirror_path}"
        )),
        _ => None,
    }
}

fn normalize_wheel_version(version: &str) -> String {
    version.trim().replace('-', "_")
}

struct PythonMetadataFields {
    name: String,
    summary: String,
    author: String,
    license: String,
    requires_python: String,
    dependencies: Vec<String>,
}

fn parse_python_metadata_fields(pyproject_raw: &str) -> io::Result<PythonMetadataFields> {
    let parsed: toml::Value = toml::from_str(pyproject_raw)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    let project = parsed
        .get("project")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "missing [project] table"))?;

    let str_field = |key: &str, fallback: &str| -> String {
        project
            .get(key)
            .and_then(toml::Value::as_str)
            .unwrap_or(fallback)
            .to_owned()
    };

    let author = project
        .get("authors")
        .and_then(toml::Value::as_array)
        .and_then(|arr| arr.first())
        .and_then(toml::Value::as_table)
        .and_then(|tbl| tbl.get("name"))
        .and_then(toml::Value::as_str)
        .unwrap_or("Archont561")
        .to_owned();

    let dependencies = project
        .get("dependencies")
        .and_then(toml::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(toml::Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();

    Ok(PythonMetadataFields {
        name: str_field("name", "playtest-py"),
        summary: str_field("description", "Playtest SDK for Python"),
        author,
        license: str_field("license", "MIT"),
        requires_python: str_field("requires-python", ">=3.10"),
        dependencies,
    })
}

fn build_wheel_metadata_text(
    fields: &PythonMetadataFields,
    version: &str,
    readme_body: &str,
) -> String {
    let mut meta = String::new();
    let _ = writeln!(meta, "Metadata-Version: 2.1");
    let _ = writeln!(meta, "Name: {}", fields.name);
    let _ = writeln!(meta, "Version: {version}");
    let _ = writeln!(meta, "Summary: {}", fields.summary);
    let _ = writeln!(meta, "Author: {}", fields.author);
    let _ = writeln!(meta, "License: {}", fields.license);
    let _ = writeln!(meta, "Requires-Python: {}", fields.requires_python);
    for dep in &fields.dependencies {
        let _ = writeln!(meta, "Requires-Dist: {dep}");
    }
    let _ = writeln!(meta, "Description-Content-Type: text/markdown");
    let _ = writeln!(meta);
    meta.push_str(readme_body.trim_end());
    meta.push('\n');
    meta
}

fn render_python_wheel(version: &str) -> io::Result<RenderedMirror> {
    let pyproject_file =
        get_embedded_file(SdkLanguage::Python, "pyproject.toml").ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "missing embedded python/pyproject.toml",
            )
        })?;
    let pyproject_str = pyproject_file
        .as_utf8()
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    let readme_str = get_embedded_file(SdkLanguage::Python, "README.md")
        .and_then(|file| file.as_utf8().ok())
        .unwrap_or("# playtest-py\n");

    let fields = parse_python_metadata_fields(pyproject_str)?;
    let wheel_ver = normalize_wheel_version(version);
    let dist_info = format!("playtest_py-{wheel_ver}.dist-info");

    let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for asset in embedded_files(SdkLanguage::Python) {
        if let Some(pkg_rel) = asset.path.strip_prefix("src/") {
            entries.insert(pkg_rel.to_owned(), asset.contents.to_vec());
        }
    }

    let metadata_bytes = build_wheel_metadata_text(&fields, &wheel_ver, readme_str).into_bytes();
    let wheel_tag_bytes = format!(
        "Wheel-Version: 1.0\nGenerator: playtest-cli ({version})\nRoot-Is-Purelib: true\nTag: py3-none-any\n"
    )
    .into_bytes();

    entries.insert(format!("{dist_info}/METADATA"), metadata_bytes);
    entries.insert(format!("{dist_info}/WHEEL"), wheel_tag_bytes);

    let record_path = format!("{dist_info}/RECORD");
    let mut record_text = String::new();
    for (entry_path, data) in &entries {
        let digest = Sha256::digest(data);
        let b64 = base64url_nopad(&digest);
        let _ = writeln!(record_text, "{entry_path},sha256={b64},{}", data.len());
    }
    let _ = writeln!(record_text, "{record_path},,");
    entries.insert(record_path, record_text.into_bytes());

    let bytes = deterministic_stored_zip(&entries);
    let sha256 = sha256_hex(&bytes);
    let filename = format!("playtest_py-{wheel_ver}-py3-none-any.whl");

    Ok(RenderedMirror {
        language: "python",
        filename,
        bytes,
        sha256,
    })
}

fn sanitize_typescript_mirror_package_json(raw_json: &str, version: &str) -> io::Result<Vec<u8>> {
    let parsed: JsonValue = serde_json::from_str(raw_json)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    let JsonValue::Object(mut obj) = parsed else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "embedded typescript/package.json must be a JSON object",
        ));
    };

    // Spec 0013 §4 rule 3: a mirror holds the SDK only (no CLI binary, shims, or dev scripts).
    obj.insert("version".to_owned(), JsonValue::String(version.to_owned()));
    obj.remove("bin");
    obj.remove("optionalDependencies");
    obj.remove("devDependencies");
    obj.remove("scripts");

    // Normalize through BTreeMap-backed sorted object keys for deterministic JSON serialization.
    let ordered: BTreeMap<String, JsonValue> = obj.into_iter().collect();
    let mut normalized_map = Map::new();
    for (key, val) in ordered {
        normalized_map.insert(key, val);
    }
    let mut text = serde_json::to_string_pretty(&JsonValue::Object(normalized_map))
        .map_err(io::Error::other)?;
    text.push('\n');
    Ok(text.into_bytes())
}

fn render_typescript_tarball(version: &str) -> io::Result<RenderedMirror> {
    let mut entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for asset in embedded_files(SdkLanguage::TypeScript) {
        let archive_path = format!("package/{}", asset.path);
        if asset.path == "package.json" {
            let raw_str = asset
                .as_utf8()
                .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
            entries.insert(
                archive_path,
                sanitize_typescript_mirror_package_json(raw_str, version)?,
            );
        } else {
            entries.insert(archive_path, asset.contents.to_vec());
        }
    }

    let mut tar_builder = TarBuilder::new(Vec::new());
    for (archive_path, contents) in &entries {
        let mut header = TarHeader::new_ustar();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(DETERMINISTIC_TAR_MTIME);
        header.set_entry_type(EntryType::Regular);
        header.set_cksum();
        tar_builder.append_data(&mut header, archive_path, contents.as_slice())?;
    }
    let tar_bytes = tar_builder.into_inner()?;
    let bytes = deterministic_gzip(&tar_bytes);
    let sha256 = sha256_hex(&bytes);
    let filename = format!("archont561-playtest-{version}.tgz");

    Ok(RenderedMirror {
        language: "typescript",
        filename,
        bytes,
        sha256,
    })
}

/// Render the deterministic SDK mirror for `language` (`"python"` or `"typescript"`) at `version`.
pub fn render_mirror(language: &str, version: &str) -> Result<RenderedMirror, String> {
    match language {
        "python" => render_python_wheel(version).map_err(|err| {
            format!("failed to render Python SDK mirror wheel for version {version}: {err}")
        }),
        "typescript" => render_typescript_tarball(version).map_err(|err| {
            format!("failed to render TypeScript SDK mirror tarball for version {version}: {err}")
        }),
        other => Err(format!(
            "unsupported mirror language `{other}` in `playtest.toml` (supported in v0.1: `python`, `typescript`)"
        )),
    }
}
