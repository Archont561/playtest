//! Embedded polyglot SDK assets bundled into the `playtest` CLI (`Spec 0012 §2`).
//!
//! `playtest-assets` embeds the exact published file set of every Playtest language SDK
//! at compile time via `build.rs` + `include_dir`, keeping each language directory as
//! the single source of truth in Git.

use std::fmt;
use std::str::{self, FromStr};

use include_dir::{Dir, DirEntry, include_dir};

/// Synchronized workspace semantic version of `playtest-assets` and every embedded SDK.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

static EMBEDDED_SDKS: Dir<'_> = include_dir!("$OUT_DIR/embedded_sdks");

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

/// Return the embedded SDK version for `language` (always equal to [`VERSION`]).
#[must_use]
pub const fn sdk_version(_language: SdkLanguage) -> &'static str {
    VERSION
}

fn strip_language_prefix(full_path: &'static str, lang_key: &str) -> &'static str {
    let normalized = full_path.strip_prefix(lang_key).unwrap_or(full_path);
    normalized
        .strip_prefix('/')
        .or_else(|| normalized.strip_prefix('\\'))
        .unwrap_or(normalized)
}

fn collect_dir_files(dir: &'static Dir<'static>, lang_key: &str, out: &mut Vec<EmbeddedFile>) {
    for entry in dir.entries() {
        match entry {
            DirEntry::Dir(sub) => collect_dir_files(sub, lang_key, out),
            DirEntry::File(file) => {
                if let Some(raw_path) = file.path().to_str() {
                    out.push(EmbeddedFile {
                        path: strip_language_prefix(raw_path, lang_key),
                        contents: file.contents(),
                    });
                }
            }
        }
    }
}

/// Return all embedded published files for `language`, sorted lexicographically by relative path.
#[must_use]
pub fn embedded_files(language: SdkLanguage) -> Vec<EmbeddedFile> {
    let lang_key = language.as_str();
    let Some(lang_dir) = EMBEDDED_SDKS.get_dir(lang_key) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    collect_dir_files(lang_dir, lang_key, &mut files);
    files.sort_by_key(|item| item.path);
    files
}

/// Look up a single embedded file for `language` by its relative forward-slash path.
#[must_use]
pub fn get_embedded_file(language: SdkLanguage, relative_path: &str) -> Option<EmbeddedFile> {
    let clean_rel = relative_path.trim_start_matches('/');
    let full_lookup = format!("{}/{clean_rel}", language.as_str());
    let file = EMBEDDED_SDKS.get_file(full_lookup)?;
    let raw_path = file.path().to_str()?;
    Some(EmbeddedFile {
        path: strip_language_prefix(raw_path, language.as_str()),
        contents: file.contents(),
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
