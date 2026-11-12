//! Human-readable (`pretty`) and machine-consumable (`json`) CLI output models
//! (`Spec 0008 §7`, `Spec 0013 §§2, 6`).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io;
use std::path::PathBuf;
use std::str::FromStr;

use serde::Serialize;

/// Version of the CLI `--format json` output schema (`Spec 0008 §7`).
pub const JSON_SCHEMA_VERSION: u32 = 1;

/// Output rendering format selected via `--format <FMT>` (`Spec 0008 §§3, 7`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum OutputFormat {
    /// Human-readable plain text output (`pretty`, default).
    #[default]
    Pretty,
    /// Machine-consumable JSON document with `schema_version` (`json`).
    Json,
}

impl OutputFormat {
    /// Canonical lowercase flag value.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pretty => "pretty",
            Self::Json => "json",
        }
    }
}

impl FromStr for OutputFormat {
    type Err = String;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "pretty" => Ok(Self::Pretty),
            "json" => Ok(Self::Json),
            other => Err(format!(
                "unsupported `--format` value `{other}` (expected `pretty` or `json`)"
            )),
        }
    }
}

/// Exit-code map included in machine output so automated consumers can inspect the contract (`Spec 0008 §7`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExitCodeSummary {
    /// Exit status `0`: command succeeded.
    pub ok: u8,
    /// Exit status `1`: at least one test target failed.
    pub test_failure: u8,
    /// Exit status `2`: CLI usage or selection error.
    pub usage_error: u8,
    /// Exit status `3`: environment or version-pin failure.
    pub environment_failure: u8,
}

/// Machine-readable reference to the Playtest CLI contract (`Spec 0008 §7`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContractSummary {
    /// Specification anchor defining the CLI contract.
    pub spec: &'static str,
    /// Consumer workspace configuration anchor.
    pub workspace_spec: &'static str,
    /// Canonical exit-code mapping.
    pub exit_codes: ExitCodeSummary,
}

impl Default for ContractSummary {
    fn default() -> Self {
        Self {
            spec: "Spec 0008 §7",
            workspace_spec: "Spec 0013 §§1-6",
            exit_codes: ExitCodeSummary {
                ok: 0,
                test_failure: 1,
                usage_error: 2,
                environment_failure: 3,
            },
        }
    }
}

/// Workspace pin status observed during `playtest version` (`Spec 0013 §6.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WorkspacePinStatus {
    /// Path to the discovered `playtest.toml`.
    pub manifest_path: PathBuf,
    /// Version pinned in `playtest.toml`.
    pub pinned_version: String,
    /// Whether `pinned_version` matches the running CLI binary version.
    pub matches_cli: bool,
}

/// Structured payload produced by `playtest version` (`Spec 0008 §§3, 7`, `Spec 0013 §6.3`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VersionReport {
    /// JSON schema version (`1`).
    pub schema_version: u32,
    /// Version of the running `playtest` CLI binary.
    pub cli_version: String,
    /// Filesystem path of the running `playtest` binary.
    pub binary_path: PathBuf,
    /// Version of the Rust `playtest` crate (`playtest-core`).
    pub rust_sdk_version: String,
    /// Versions of every embedded polyglot SDK (`python`, `typescript`, `cpp`, `c`, `csharp`, `go`, `java`, `r`).
    pub embedded_sdks: BTreeMap<String, String>,
    /// Discovered workspace pin status, if a valid `playtest.toml` exists at or above `cwd`.
    pub workspace: Option<WorkspacePinStatus>,
    /// Contract anchor and exit-code summary (`Spec 0008 §7`).
    pub contract: ContractSummary,
}

impl VersionReport {
    /// Render `playtest version` in human-readable `pretty` format.
    #[must_use]
    pub fn render_pretty(&self) -> String {
        let mut buf = String::new();
        let _ = writeln!(buf, "playtest {}", self.cli_version);
        let _ = writeln!(buf, "binary: {}", self.binary_path.display());
        match &self.workspace {
            Some(ws) if ws.matches_cli => {
                let _ = writeln!(
                    buf,
                    "workspace: {} (pinned {})",
                    ws.manifest_path.display(),
                    ws.pinned_version
                );
            }
            Some(ws) => {
                let _ = writeln!(
                    buf,
                    "workspace: {} (pinned {}, mismatch)",
                    ws.manifest_path.display(),
                    ws.pinned_version
                );
            }
            None => {
                let _ = writeln!(buf, "workspace: none");
            }
        }
        let _ = writeln!(buf, "rust sdk: {}", self.rust_sdk_version);
        let _ = writeln!(buf, "embedded sdks:");
        for (lang, ver) in &self.embedded_sdks {
            let _ = writeln!(buf, "  {lang}: {ver}");
        }
        let _ = writeln!(
            buf,
            "contract: {} / {} (exit codes: 0 ok, 1 test failure, 2 usage error, 3 environment failure; use `--format json` for machine output)",
            self.contract.spec, self.contract.workspace_spec
        );
        buf
    }

    /// Render `playtest version` as pretty-printed JSON with trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] if JSON serialization fails.
    pub fn render_json(&self) -> io::Result<String> {
        let mut json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        json.push('\n');
        Ok(json)
    }
}

/// Structured payload produced by `playtest init` (`Spec 0013 §2`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InitReport {
    /// JSON schema version (`1`).
    pub schema_version: u32,
    /// Root directory of the initialized workspace.
    pub workspace_root: PathBuf,
    /// Path to `<workspace_root>/playtest.toml`.
    pub manifest_path: PathBuf,
    /// Path to `<workspace_root>/.playtest/.gitignore`.
    pub gitignore_path: PathBuf,
    /// Version pinned in `playtest.toml`.
    pub version: String,
    /// Configured `[mirrors].languages` list in `playtest.toml`.
    pub languages: Vec<String>,
    /// `true` when `playtest.toml` was newly created; `false` when an existing file was kept.
    pub created_manifest: bool,
    /// `true` when `.playtest/.gitignore` was newly created; `false` when an existing file was kept.
    pub created_gitignore: bool,
    /// Ordered follow-up commands (`playtest sync` and ecosystem wiring commands from `Spec 0013 §5`).
    pub next_steps: Vec<String>,
}

impl InitReport {
    /// Render `playtest init` in human-readable `pretty` format.
    #[must_use]
    pub fn render_pretty(&self) -> String {
        let mut buf = String::new();
        if self.created_manifest || self.created_gitignore {
            let _ = writeln!(
                buf,
                "Initialized Playtest workspace in {} (pinned to {})",
                self.workspace_root.display(),
                self.version
            );
        } else {
            let _ = writeln!(
                buf,
                "Playtest workspace already initialized in {} (pinned to {})",
                self.workspace_root.display(),
                self.version
            );
        }
        let manifest_state = if self.created_manifest {
            "created"
        } else {
            "unchanged"
        };
        let gitignore_state = if self.created_gitignore {
            "created"
        } else {
            "unchanged"
        };
        let _ = writeln!(
            buf,
            "  {}: {}",
            self.manifest_path.display(),
            manifest_state
        );
        let _ = writeln!(
            buf,
            "  {}: {}",
            self.gitignore_path.display(),
            gitignore_state
        );
        let _ = writeln!(buf, "Next steps:");
        for step in &self.next_steps {
            let _ = writeln!(buf, "  {step}");
        }
        buf
    }

    /// Render `playtest init` as pretty-printed JSON with trailing newline.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] if JSON serialization fails.
    pub fn render_json(&self) -> io::Result<String> {
        let mut json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        json.push('\n');
        Ok(json)
    }
}

/// Unified command output rendered according to [`OutputFormat`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommandOutput {
    Version(VersionReport),
    Init(InitReport),
}

impl CommandOutput {
    pub(crate) fn render(&self, format: OutputFormat) -> io::Result<String> {
        match (self, format) {
            (Self::Version(report), OutputFormat::Pretty) => Ok(report.render_pretty()),
            (Self::Version(report), OutputFormat::Json) => report.render_json(),
            (Self::Init(report), OutputFormat::Pretty) => Ok(report.render_pretty()),
            (Self::Init(report), OutputFormat::Json) => report.render_json(),
        }
    }
}
