//! `playtest version` implementation (`Spec 0008 §§3, 7`, `Spec 0013 §6.3`).

use std::collections::BTreeMap;

use playtest_assets::all_embedded_sdks;

use crate::RunEnvironment;
use crate::output::{ContractSummary, JSON_SCHEMA_VERSION, VersionReport, WorkspacePinStatus};

/// Build the [`VersionReport`] for `env` and an optional discovered workspace status.
#[must_use]
pub(crate) fn build_version_report(
    env: &RunEnvironment,
    workspace: Option<WorkspacePinStatus>,
) -> VersionReport {
    let mut embedded_sdks = BTreeMap::new();
    for sdk in all_embedded_sdks() {
        embedded_sdks.insert(sdk.language.as_str().to_owned(), sdk.version.to_owned());
    }

    VersionReport {
        schema_version: JSON_SCHEMA_VERSION,
        cli_version: env.cli_version.clone(),
        binary_path: env.binary_path.clone(),
        rust_sdk_version: playtest_core::VERSION.to_owned(),
        embedded_sdks,
        workspace,
        contract: ContractSummary::default(),
    }
}
