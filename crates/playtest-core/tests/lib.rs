use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};

use playtest_core::{
    DEFAULT_MIRROR_LANGUAGES, ExitStatus, WORKSPACE_GITIGNORE_CONTENTS, WORKSPACE_MANIFEST_FILE,
    WORKSPACE_SCHEMA_VERSION, WorkspaceConfig, WorkspaceError, discover_workspace,
    find_workspace_manifest, require_workspace, verify_workspace_pin,
};
use rstest::rstest;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(label: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let dir = env::temp_dir().join(format!(
            "playtest-core-test-{}-{label}-{id}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create scratch dir");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[rstest]
fn test_version_accessible() {
    assert_eq!(playtest_core::VERSION, "0.1.0");
}

#[rstest]
fn test_exit_status_codes_follow_spec_0008() {
    assert_eq!(ExitStatus::Success.as_u8(), 0);
    assert_eq!(ExitStatus::TestFailure.as_u8(), 1);
    assert_eq!(ExitStatus::UsageError.as_u8(), 2);
    assert_eq!(ExitStatus::EnvironmentFailure.as_u8(), 3);

    assert_eq!(ExitStatus::Success.as_i32(), 0);
    assert_eq!(ExitStatus::EnvironmentFailure.as_i32(), 3);
    assert_eq!(ExitCode::from(ExitStatus::Success), ExitCode::SUCCESS);
}

#[rstest]
fn test_workspace_config_roundtrip_and_canonical_toml() {
    let cfg = WorkspaceConfig::default_for_version("0.1.0");
    assert_eq!(cfg.schema, WORKSPACE_SCHEMA_VERSION);
    assert_eq!(cfg.version, "0.1.0");
    assert_eq!(cfg.mirrors.languages, DEFAULT_MIRROR_LANGUAGES);

    let rendered = cfg.to_toml_string();
    assert_eq!(
        rendered,
        "schema = 1\nversion = \"0.1.0\"\n\n[mirrors]\nlanguages = [\"python\", \"typescript\"]\n"
    );

    let parsed = WorkspaceConfig::from_toml_str(&rendered, Path::new("playtest.toml"))
        .expect("canonical playtest.toml parses");
    assert_eq!(parsed, cfg);
}

#[rstest]
fn test_workspace_config_rejects_invalid_shapes() {
    let path = Path::new("/repo/playtest.toml");

    let unsupported = WorkspaceConfig::from_toml_str("schema = 99\nversion = \"0.1.0\"\n", path)
        .expect_err("schema 99 must fail");
    assert_eq!(
        unsupported,
        WorkspaceError::UnsupportedSchema {
            manifest_path: path.to_path_buf(),
            found: 99,
            supported: 1,
        }
    );

    let empty_ver = WorkspaceConfig::from_toml_str("schema = 1\nversion = \"   \"\n", path)
        .expect_err("blank version must fail");
    assert_eq!(
        empty_ver,
        WorkspaceError::EmptyVersionPin {
            manifest_path: path.to_path_buf(),
        }
    );

    let unknown_key =
        WorkspaceConfig::from_toml_str("schema = 1\nversion = \"0.1.0\"\nextra = true\n", path)
            .expect_err("unknown top-level key must fail");
    assert!(matches!(
        unknown_key,
        WorkspaceError::InvalidManifestToml { .. }
    ));
}

#[rstest]
fn test_workspace_discovery_matching_mismatched_and_missing() {
    let scratch = ScratchDir::new("discovery");
    let nested = scratch.path().join("packages/service");
    fs::create_dir_all(&nested).expect("create nested dir");
    let fake_bin = Path::new("/opt/bin/playtest");

    // Missing playtest.toml
    assert_eq!(find_workspace_manifest(&nested), None);
    assert_eq!(
        discover_workspace(&nested).expect("discover"),
        None,
        "no workspace before init"
    );
    assert_eq!(
        verify_workspace_pin(&nested, "0.1.0", fake_bin).expect("verify outside workspace"),
        None
    );
    let missing_err =
        require_workspace(&nested, "0.1.0", fake_bin).expect_err("require_workspace fails");
    assert!(matches!(
        missing_err,
        WorkspaceError::MissingManifest { .. }
    ));
    assert!(missing_err.to_string().contains("playtest init"));

    // Write matching playtest.toml at root
    let manifest_path = scratch.path().join(WORKSPACE_MANIFEST_FILE);
    fs::write(
        &manifest_path,
        WorkspaceConfig::default_for_version("0.1.0").to_toml_string(),
    )
    .expect("write playtest.toml");

    let discovered = require_workspace(&nested, "0.1.0", fake_bin).expect("matching pin");
    assert_eq!(discovered.root, scratch.path());
    assert_eq!(discovered.manifest_path, manifest_path);
    assert_eq!(discovered.config.version, "0.1.0");

    // Mismatched pin
    let mismatch_err =
        verify_workspace_pin(&nested, "0.2.0", fake_bin).expect_err("mismatched pin");
    let rendered_err = mismatch_err.to_string();
    assert!(rendered_err.contains("0.1.0"));
    assert!(rendered_err.contains("0.2.0"));
    assert!(rendered_err.contains("/opt/bin/playtest"));
    assert!(rendered_err.contains("playtest sync"));

    assert!(WORKSPACE_GITIGNORE_CONTENTS.contains("!*.whl"));
    assert!(WORKSPACE_GITIGNORE_CONTENTS.contains("!*.tgz"));
}
