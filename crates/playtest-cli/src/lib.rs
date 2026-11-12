//! # playtest-cli
//!
//! Command-line interface library for the unified `playtest` binary
//! (`Spec 0008`, `Spec 0012 §1`, `Spec 0013 §§1-6`).
//!
//! Command-line parser internals remain private so neither `playtest-core` nor
//! documented CLI interfaces expose third-party CLI framework types (`Spec 0012 §1`).

mod cli;
mod commands;
pub mod mirrors;
pub mod output;

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

pub use mirrors::{
    RenderedMirror, expected_mirror_filename, render_mirror, sha256_hex, wiring_command_for,
};
pub use output::{
    ContractSummary, ExitCodeSummary, InitReport, JSON_SCHEMA_VERSION, MirrorDriftEntry,
    MirrorDriftKind, MirrorSyncStatus, OutputFormat, SyncCheckReport, SyncReport,
    SyncedMirrorEntry, VersionReport, WorkspacePinStatus,
};
pub use playtest_core::ExitStatus;

/// Current CLI crate version (`env!("CARGO_PKG_VERSION")`).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Host execution parameters supplied to a CLI invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunEnvironment {
    /// Working directory from which workspace discovery starts.
    pub cwd: PathBuf,
    /// Path of the currently executing `playtest` binary.
    pub binary_path: PathBuf,
    /// Version reported by the running `playtest` binary (defaults to [`VERSION`]).
    pub cli_version: String,
}

impl RunEnvironment {
    /// Construct a [`RunEnvironment`] for `cwd` and `binary_path` at [`VERSION`].
    #[must_use]
    pub fn new(cwd: impl Into<PathBuf>, binary_path: impl Into<PathBuf>) -> Self {
        Self {
            cwd: cwd.into(),
            binary_path: binary_path.into(),
            cli_version: VERSION.to_owned(),
        }
    }

    /// Override the simulated CLI version (used by version-pin and upgrade conformance tests).
    #[must_use]
    pub fn with_cli_version(mut self, cli_version: impl Into<String>) -> Self {
        self.cli_version = cli_version.into();
        self
    }

    /// Capture the current OS working directory and executable path.
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] if the working directory or executable path cannot be queried.
    pub fn detect() -> io::Result<Self> {
        let cwd = env::current_dir()?;
        let binary_path = env::current_exe()?;
        Ok(Self::new(cwd, binary_path))
    }
}

/// Execute `argv` against an explicit [`RunEnvironment`] and output streams.
pub fn run_with_env<I, T>(
    argv: I,
    env: &RunEnvironment,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> ExitStatus
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    cli::execute_argv(argv, env, out, err)
}

/// Run the `playtest` process entrypoint using OS arguments, streams, and host environment.
#[must_use]
pub fn run_process() -> ExitCode {
    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    let env = match RunEnvironment::detect() {
        Ok(env) => env,
        Err(detect_err) => {
            let _ = writeln!(
                stderr,
                "playtest: failed to inspect host environment: {detect_err}"
            );
            let _ = stderr.flush();
            return ExitCode::from(ExitStatus::EnvironmentFailure);
        }
    };
    let status = run_with_env(env::args_os(), &env, &mut stdout, &mut stderr);
    let _ = stderr.flush();
    let _ = stdout.flush();
    ExitCode::from(status)
}
