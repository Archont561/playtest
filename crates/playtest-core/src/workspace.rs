//! Workspace manifest (`playtest.toml`) schema, discovery, and version-pin verification
//! (`Spec 0013 §§1, 6` and `Spec 0008 §7`).

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Canonical workspace manifest filename at the root of a Playtest workspace (`Spec 0013 §1`).
pub const WORKSPACE_MANIFEST_FILE: &str = "playtest.toml";

/// Directory owned by Playtest inside a consumer workspace (`Spec 0013 §2`).
pub const WORKSPACE_DIR_NAME: &str = ".playtest";

/// Gitignore filename inside `.playtest/` that re-includes mirror artifacts (`Spec 0013 §2`).
pub const WORKSPACE_GITIGNORE_FILE: &str = ".gitignore";

/// Current `playtest.toml` schema version (`Spec 0013 §1`).
pub const WORKSPACE_SCHEMA_VERSION: u32 = 1;

/// Default SDK mirror languages initialized in Milestone M1 (`Spec 0013 §§1, 4`).
pub const DEFAULT_MIRROR_LANGUAGES: [&str; 2] = ["python", "typescript"];

/// Contents written to `.playtest/.gitignore` by `playtest init` (`Spec 0013 §2.2`).
pub const WORKSPACE_GITIGNORE_CONTENTS: &str = "\
# Managed by Playtest (Spec 0013 §2).
# Re-include committed SDK mirror artifacts when parent ignore rules hide them.
!*.whl
!*.tgz
";

/// Process exit codes defined by the Playtest CLI machine contract (`Spec 0008 §7`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ExitStatus {
    /// Command completed successfully (`0`).
    Success = 0,
    /// At least one executed test target failed (`1`).
    TestFailure = 1,
    /// CLI usage, flag, or explicit selection error (`2`).
    UsageError = 2,
    /// Environment failure: version-pin mismatch, missing/invalid workspace, or I/O failure (`3`).
    EnvironmentFailure = 3,
}

impl ExitStatus {
    /// Return the numeric `u8` exit status code.
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Return the numeric `i32` exit status code.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self as i32
    }
}

impl From<ExitStatus> for ExitCode {
    fn from(status: ExitStatus) -> Self {
        Self::from(status.as_u8())
    }
}

/// `[mirrors]` table inside `playtest.toml` (`Spec 0013 §1`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MirrorsConfig {
    /// Canonical language identifiers whose SDK mirrors are managed in `.playtest/<lang>/`.
    #[serde(default)]
    pub languages: Vec<String>,
}

/// Parsed and validated `playtest.toml` workspace configuration (`Spec 0013 §1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    /// Manifest schema version (`1`).
    pub schema: u32,
    /// Exact Playtest SemVer version pinned by this workspace.
    pub version: String,
    /// Mirror settings under `[mirrors]`.
    #[serde(default)]
    pub mirrors: MirrorsConfig,
}

impl WorkspaceConfig {
    /// Construct a `WorkspaceConfig` at [`WORKSPACE_SCHEMA_VERSION`] for `version` and `languages`.
    #[must_use]
    pub fn new(version: impl Into<String>, languages: Vec<String>) -> Self {
        Self {
            schema: WORKSPACE_SCHEMA_VERSION,
            version: version.into(),
            mirrors: MirrorsConfig { languages },
        }
    }

    /// Construct the default `playtest.toml` configuration for `version` (`Spec 0013 §§1, 2`).
    #[must_use]
    pub fn default_for_version(version: impl Into<String>) -> Self {
        let languages = DEFAULT_MIRROR_LANGUAGES
            .iter()
            .map(|lang| (*lang).to_owned())
            .collect();
        Self::new(version, languages)
    }

    /// Parse and validate `playtest.toml` text from `manifest_path`.
    ///
    /// # Errors
    ///
    /// Returns [`WorkspaceError`] when TOML parsing fails, `schema` is unsupported,
    /// or `version` is blank.
    pub fn from_toml_str(raw: &str, manifest_path: &Path) -> Result<Self, WorkspaceError> {
        let parsed: Self =
            toml::from_str(raw).map_err(|err| WorkspaceError::InvalidManifestToml {
                manifest_path: manifest_path.to_path_buf(),
                reason: err.to_string(),
            })?;
        if parsed.schema != WORKSPACE_SCHEMA_VERSION {
            return Err(WorkspaceError::UnsupportedSchema {
                manifest_path: manifest_path.to_path_buf(),
                found: parsed.schema,
                supported: WORKSPACE_SCHEMA_VERSION,
            });
        }
        if parsed.version.trim().is_empty() {
            return Err(WorkspaceError::EmptyVersionPin {
                manifest_path: manifest_path.to_path_buf(),
            });
        }
        Ok(parsed)
    }

    /// Render `playtest.toml` in canonical Taplo-formatted layout (`Spec 0013 §1`).
    #[must_use]
    pub fn to_toml_string(&self) -> String {
        let quoted_langs: Vec<String> = self
            .mirrors
            .languages
            .iter()
            .map(|lang| format!("{lang:?}"))
            .collect();
        let langs_inline = quoted_langs.join(", ");
        format!(
            "schema = {}\nversion = {:?}\n\n[mirrors]\nlanguages = [{}]\n",
            self.schema, self.version, langs_inline
        )
    }
}

/// Discovered workspace root, manifest path, and parsed `playtest.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredWorkspace {
    /// Directory containing `playtest.toml`.
    pub root: PathBuf,
    /// Full path to `<root>/playtest.toml`.
    pub manifest_path: PathBuf,
    /// Validated `playtest.toml` configuration.
    pub config: WorkspaceConfig,
}

/// Detailed diagnostic for a `playtest.toml` version-pin mismatch (`Spec 0013 §6`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionPinMismatch {
    /// Path to the `playtest.toml` declaring the pin.
    pub manifest_path: PathBuf,
    /// Pinned version declared in `playtest.toml`.
    pub pinned_version: String,
    /// Version of the currently executing `playtest` binary.
    pub running_version: String,
    /// Filesystem path of the currently executing `playtest` binary.
    pub binary_path: PathBuf,
}

impl VersionPinMismatch {
    /// Return the two actionable remediation options mandated by `Spec 0013 §6.2`.
    #[must_use]
    pub fn fixes(&self) -> [String; 2] {
        [
            format!(
                "install playtest {} so PATH resolves the pinned binary",
                self.pinned_version
            ),
            format!(
                "update version = {:?} in {} and run `playtest sync`",
                self.running_version,
                self.manifest_path.display()
            ),
        ]
    }
}

impl fmt::Display for VersionPinMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [fix_install, fix_update] = self.fixes();
        write!(
            f,
            "workspace version pin mismatch in {}\n  pinned version: {}\n  running binary: {} ({})\n  fix 1: {}\n  fix 2: {}",
            self.manifest_path.display(),
            self.pinned_version,
            self.running_version,
            self.binary_path.display(),
            fix_install,
            fix_update,
        )
    }
}

/// Errors raised while locating, parsing, or verifying a Playtest workspace (`ExitStatus::EnvironmentFailure`).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum WorkspaceError {
    /// No `playtest.toml` exists at or above the working directory.
    #[error(
        "no `{WORKSPACE_MANIFEST_FILE}` found at or above {}; run `playtest init` to initialize a workspace",
        search_start.display()
    )]
    MissingManifest {
        /// Starting directory from which upward discovery ran.
        search_start: PathBuf,
    },

    /// `playtest.toml` could not be read from disk.
    #[error("failed to read {}: {reason}", manifest_path.display())]
    Io {
        /// Path to `playtest.toml`.
        manifest_path: PathBuf,
        /// Underlying OS error description.
        reason: String,
    },

    /// `playtest.toml` failed TOML syntax or field validation.
    #[error("invalid `{WORKSPACE_MANIFEST_FILE}` at {}: {reason}", manifest_path.display())]
    InvalidManifestToml {
        /// Path to `playtest.toml`.
        manifest_path: PathBuf,
        /// Parser failure details.
        reason: String,
    },

    /// `playtest.toml` declared an unsupported `schema` integer.
    #[error(
        "unsupported `{WORKSPACE_MANIFEST_FILE}` schema {found} in {} (supported schema: {supported})",
        manifest_path.display()
    )]
    UnsupportedSchema {
        /// Path to `playtest.toml`.
        manifest_path: PathBuf,
        /// Observed `schema` number.
        found: u32,
        /// Supported `schema` number.
        supported: u32,
    },

    /// `playtest.toml` declared an empty `version` string.
    #[error("empty `version` pin in {}", manifest_path.display())]
    EmptyVersionPin {
        /// Path to `playtest.toml`.
        manifest_path: PathBuf,
    },

    /// Running binary version differs from `version` in `playtest.toml` (`Spec 0013 §6`).
    #[error("{0}")]
    PinMismatch(VersionPinMismatch),
}

/// Walk `start_dir` and its parent directories to find the nearest `playtest.toml`.
#[must_use]
pub fn find_workspace_manifest(start_dir: &Path) -> Option<PathBuf> {
    start_dir
        .ancestors()
        .map(|ancestor| ancestor.join(WORKSPACE_MANIFEST_FILE))
        .find(|candidate| candidate.is_file())
}

/// Discover and parse the nearest `playtest.toml` at or above `start_dir`, if one exists.
///
/// # Errors
///
/// Returns [`WorkspaceError`] if a `playtest.toml` file is found but cannot be read or is invalid.
pub fn discover_workspace(start_dir: &Path) -> Result<Option<DiscoveredWorkspace>, WorkspaceError> {
    let Some(manifest_path) = find_workspace_manifest(start_dir) else {
        return Ok(None);
    };
    let raw = fs::read_to_string(&manifest_path).map_err(|err| WorkspaceError::Io {
        manifest_path: manifest_path.clone(),
        reason: err.to_string(),
    })?;
    let config = WorkspaceConfig::from_toml_str(&raw, &manifest_path)?;
    let root = manifest_path
        .parent()
        .map_or_else(|| start_dir.to_path_buf(), Path::to_path_buf);
    Ok(Some(DiscoveredWorkspace {
        root,
        manifest_path,
        config,
    }))
}

/// Verify that if `start_dir` is inside a workspace, its `version` pin matches `running_version`
/// (`Spec 0013 §6.1`).
///
/// Returns `Ok(None)` when `start_dir` is outside any workspace (no `playtest.toml` found).
///
/// # Errors
///
/// Returns [`WorkspaceError`] if `playtest.toml` is malformed or its `version` pin differs from
/// `running_version`.
pub fn verify_workspace_pin(
    start_dir: &Path,
    running_version: &str,
    binary_path: &Path,
) -> Result<Option<DiscoveredWorkspace>, WorkspaceError> {
    let Some(workspace) = discover_workspace(start_dir)? else {
        return Ok(None);
    };
    if workspace.config.version != running_version {
        return Err(WorkspaceError::PinMismatch(VersionPinMismatch {
            manifest_path: workspace.manifest_path,
            pinned_version: workspace.config.version,
            running_version: running_version.to_owned(),
            binary_path: binary_path.to_path_buf(),
        }));
    }
    Ok(Some(workspace))
}

/// Require `start_dir` to be inside a valid workspace whose pin matches `running_version`.
///
/// # Errors
///
/// Returns [`WorkspaceError::MissingManifest`] when no `playtest.toml` exists at or above
/// `start_dir`, or [`WorkspaceError`] when the manifest is invalid or mismatched.
pub fn require_workspace(
    start_dir: &Path,
    running_version: &str,
    binary_path: &Path,
) -> Result<DiscoveredWorkspace, WorkspaceError> {
    verify_workspace_pin(start_dir, running_version, binary_path)?.ok_or_else(|| {
        WorkspaceError::MissingManifest {
            search_start: start_dir.to_path_buf(),
        }
    })
}
