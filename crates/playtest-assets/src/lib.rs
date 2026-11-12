//! Embedded polyglot SDK assets bundled into the `playtest` CLI (`Spec 0012 §2`, `ADR 0020`).
//!
//! `playtest-assets` packs the published file set of every Playtest language SDK into a
//! deterministic solid POSIX `ustar` `.tar.br` stream at compile time (`build.rs` + `brotli`
//! with `portable-float`) and lazily decodes it once on first access via `brotli-decompressor`
//! and [`std::sync::OnceLock`], keeping each language directory as the single source of truth
//! in Git while preserving zero-copy `'static` slices for callers.

use std::collections::BTreeMap;
use std::fmt;
use std::io::Read;
use std::str::{self, FromStr};
use std::sync::OnceLock;

use brotli_decompressor::Decompressor;

/// Synchronized workspace semantic version of `playtest-assets` and every embedded SDK.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const BROTLI_DECODE_BUFFER_SIZE: usize = 4096;

static EMBEDDED_SDKS_TAR_BR: &[u8] =
    include_bytes!(concat!(env!("OUT_DIR"), "/embedded_sdks.tar.br"));

static DECODED_SDKS: OnceLock<BTreeMap<SdkLanguage, Vec<DecodedAsset>>> = OnceLock::new();

#[derive(Debug)]
struct DecodedAsset {
    path: Box<str>,
    contents: Box<[u8]>,
}

/// Supported target SDK languages embedded inside `playtest-assets`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SdkLanguage {
    /// Python SDK (`python/`).
    Python,
    /// TypeScript SDK (`typescript/`).
    TypeScript,
    /// C++ SDK (`cpp/`).
    Cpp,
    /// C SDK (`c/`).
    C,
    /// C# / .NET SDK (`csharp/`).
    CSharp,
    /// Go SDK (`go/`).
    Go,
    /// Java SDK (`java/`).
    Java,
    /// R SDK (`r/`).
    R,
}

impl SdkLanguage {
    /// Deterministic list of all embedded SDK languages.
    pub const ALL: [Self; 8] = [
        Self::Python,
        Self::TypeScript,
        Self::Cpp,
        Self::C,
        Self::CSharp,
        Self::Go,
        Self::Java,
        Self::R,
    ];

    /// Canonical manifest key and embedded directory name for this language.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Python => "python",
            Self::TypeScript => "typescript",
            Self::Cpp => "cpp",
            Self::C => "c",
            Self::CSharp => "csharp",
            Self::Go => "go",
            Self::Java => "java",
            Self::R => "r",
        }
    }

    /// Monorepo directory name holding the canonical SDK sources.
    #[must_use]
    pub const fn source_dir(self) -> &'static str {
        self.as_str()
    }
}

impl fmt::Display for SdkLanguage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Error returned when parsing an unknown SDK language identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSdkLanguageError(pub String);

impl fmt::Display for UnknownSdkLanguageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unknown SDK language `{}`", self.0)
    }
}

impl std::error::Error for UnknownSdkLanguageError {}

impl FromStr for SdkLanguage {
    type Err = UnknownSdkLanguageError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "python" | "py" => Ok(Self::Python),
            "typescript" | "ts" => Ok(Self::TypeScript),
            "cpp" | "c++" => Ok(Self::Cpp),
            "c" => Ok(Self::C),
            "csharp" | "cs" | "c#" => Ok(Self::CSharp),
            "go" | "golang" => Ok(Self::Go),
            "java" => Ok(Self::Java),
            "r" => Ok(Self::R),
            _ => Err(UnknownSdkLanguageError(raw.to_owned())),
        }
    }
}

/// Error returned when decoding a solid `.tar.br` SDK asset stream fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveDecodeError(pub String);

impl fmt::Display for ArchiveDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to decode embedded SDK archive: {}", self.0)
    }
}

impl std::error::Error for ArchiveDecodeError {}

/// A single embedded SDK file with its relative path inside the SDK root and raw bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EmbeddedFile {
    /// Forward-slash relative path within the language SDK root (e.g. `src/playtest/__init__.py`).
    pub path: &'static str,
    /// Raw file bytes embedded at compile time.
    pub contents: &'static [u8],
}

impl EmbeddedFile {
    /// Decode the embedded file bytes as UTF-8 text.
    ///
    /// # Errors
    ///
    /// Returns [`std::str::Utf8Error`] if the embedded asset contains non-UTF-8 bytes.
    pub fn as_utf8(self) -> Result<&'static str, str::Utf8Error> {
        str::from_utf8(self.contents)
    }
}

/// Complete embedded bundle for a single language SDK (`Spec 0012 §2.6`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedSdk {
    /// Target SDK language.
    pub language: SdkLanguage,
    /// Embedded SDK version, equal to the workspace [`VERSION`].
    pub version: &'static str,
    /// Deterministically sorted published files for this SDK.
    pub files: Vec<EmbeddedFile>,
}

/// Return the raw compile-time solid `.tar.br` payload (`ADR 0020`).
#[must_use]
pub const fn embedded_archive_bytes() -> &'static [u8] {
    EMBEDDED_SDKS_TAR_BR
}

/// Validate and decode a solid `.tar.br` SDK archive, returning the total file count.
///
/// # Errors
///
/// Returns [`ArchiveDecodeError`] if Brotli decompression fails, the tar stream is
/// malformed, or an archive entry lacks a valid `<lang>/<relative_path>` prefix.
pub fn verify_archive_bytes(tar_br: &[u8]) -> Result<usize, ArchiveDecodeError> {
    let decoded = decode_solid_tar_br(tar_br)?;
    Ok(decoded.values().map(Vec::len).sum())
}

fn decode_solid_tar_br(
    tar_br: &[u8],
) -> Result<BTreeMap<SdkLanguage, Vec<DecodedAsset>>, ArchiveDecodeError> {
    let mut tar_bytes = Vec::new();
    let mut decoder = Decompressor::new(tar_br, BROTLI_DECODE_BUFFER_SIZE);
    decoder
        .read_to_end(&mut tar_bytes)
        .map_err(|err| ArchiveDecodeError(format!("brotli stream error: {err}")))?;

    let mut archive = tar::Archive::new(tar_bytes.as_slice());
    let entries = archive
        .entries()
        .map_err(|err| ArchiveDecodeError(format!("tar header error: {err}")))?;

    let mut by_lang: BTreeMap<SdkLanguage, Vec<DecodedAsset>> = BTreeMap::new();
    for entry_res in entries {
        let mut entry =
            entry_res.map_err(|err| ArchiveDecodeError(format!("tar entry error: {err}")))?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let raw_path = entry
            .path()
            .map_err(|err| ArchiveDecodeError(format!("invalid entry path: {err}")))?
            .to_string_lossy()
            .replace('\\', "/");
        let (lang_key, rel_path) = raw_path.split_once('/').ok_or_else(|| {
            ArchiveDecodeError(format!(
                "archive entry `{raw_path}` missing `<lang>/` prefix"
            ))
        })?;
        if rel_path.is_empty() {
            return Err(ArchiveDecodeError(format!(
                "archive entry `{raw_path}` has empty relative path"
            )));
        }
        let lang =
            SdkLanguage::from_str(lang_key).map_err(|err| ArchiveDecodeError(err.to_string()))?;

        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|err| ArchiveDecodeError(format!("failed reading `{raw_path}`: {err}")))?;

        by_lang.entry(lang).or_default().push(DecodedAsset {
            path: rel_path.into(),
            contents: buf.into_boxed_slice(),
        });
    }

    for files in by_lang.values_mut() {
        files.sort_by(|lhs, rhs| lhs.path.cmp(&rhs.path));
    }
    Ok(by_lang)
}

fn decoded_sdks() -> &'static BTreeMap<SdkLanguage, Vec<DecodedAsset>> {
    DECODED_SDKS.get_or_init(|| match decode_solid_tar_br(EMBEDDED_SDKS_TAR_BR) {
        Ok(map) => map,
        Err(err) => panic!("{err}"),
    })
}

/// Return the embedded SDK version for `language` (always equal to [`VERSION`]).
#[must_use]
pub const fn sdk_version(_language: SdkLanguage) -> &'static str {
    VERSION
}

/// Return all embedded published files for `language`, sorted lexicographically by relative path.
#[must_use]
pub fn embedded_files(language: SdkLanguage) -> Vec<EmbeddedFile> {
    let Some(entries) = decoded_sdks().get(&language) else {
        return Vec::new();
    };
    entries
        .iter()
        .map(|item| EmbeddedFile {
            path: &item.path,
            contents: &item.contents,
        })
        .collect()
}

/// Look up a single embedded file for `language` by its relative forward-slash path.
#[must_use]
pub fn get_embedded_file(language: SdkLanguage, relative_path: &str) -> Option<EmbeddedFile> {
    let clean_rel = relative_path.trim_start_matches('/');
    let entries = decoded_sdks().get(&language)?;
    let idx = entries
        .binary_search_by(|item| item.path.as_ref().cmp(clean_rel))
        .ok()?;
    let found = entries.get(idx)?;
    Some(EmbeddedFile {
        path: &found.path,
        contents: &found.contents,
    })
}

/// Return the embedded SDK bundle for a single `language`.
#[must_use]
pub fn embedded_sdk(language: SdkLanguage) -> EmbeddedSdk {
    EmbeddedSdk {
        language,
        version: sdk_version(language),
        files: embedded_files(language),
    }
}

/// Return the embedded SDK bundles for every supported language in [`SdkLanguage::ALL`] order.
#[must_use]
pub fn all_embedded_sdks() -> Vec<EmbeddedSdk> {
    SdkLanguage::ALL.into_iter().map(embedded_sdk).collect()
}
