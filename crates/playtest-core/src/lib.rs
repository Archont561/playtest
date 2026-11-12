//! # playtest-core
//!
//! Shared version constants, workspace configuration (`playtest.toml`), exit-status
//! contract, and report-protocol types for Playtest (`Spec 0008 §7`, `Spec 0012 §1`,
//! `Spec 0013 §§1, 6`).

pub mod workspace;

/// Current core crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub use workspace::{
    DEFAULT_MIRROR_LANGUAGES, DiscoveredWorkspace, ExitStatus, MirrorsConfig, VersionPinMismatch,
    WORKSPACE_DIR_NAME, WORKSPACE_GITIGNORE_CONTENTS, WORKSPACE_GITIGNORE_FILE,
    WORKSPACE_MANIFEST_FILE, WORKSPACE_SCHEMA_VERSION, WorkspaceConfig, WorkspaceError,
    discover_workspace, find_workspace_manifest, require_workspace, verify_workspace_pin,
};
