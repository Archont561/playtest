//! Integration and determinism conformance tests for `playtest sync` and `playtest sync --check`
//! (`Spec 0013 §§3-5, 8, 9` / `task_053`).

use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;

use flate2::read::GzDecoder;
use playtest_cli::{
    ExitStatus, JSON_SCHEMA_VERSION, MirrorDriftKind, MirrorSyncStatus, RunEnvironment, VERSION,
    expected_mirror_filename, render_mirror, run_with_env, sha256_hex, wiring_command_for,
};
use playtest_core::{WORKSPACE_DIR_NAME, WORKSPACE_GITIGNORE_FILE, WORKSPACE_MANIFEST_FILE};
use serde_json::Value as JsonValue;
use tar::Archive as TarArchive;
use tempfile::TempDir;
use zip::ZipArchive;

fn run_cli(env: &RunEnvironment, args: &[&str]) -> (ExitStatus, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let status = run_with_env(args, env, &mut out, &mut err);
    (
        status,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

fn make_env(cwd: &Path, cli_version: &str) -> RunEnvironment {
    RunEnvironment::new(cwd, cwd.join("bin").join("playtest")).with_cli_version(cli_version)
}

#[test]
fn test_sync_renders_deterministic_valid_wheel_and_tarball_without_editing_consumer_manifests() {
    let ws_a = TempDir::new().expect("tempdir a");
    let ws_b = TempDir::new().expect("tempdir b");

    let consumer_pyproject = "[project]\nname = \"demo\"\nversion = \"0.0.1\"\n";
    let consumer_pkg_json = "{\n  \"name\": \"demo-web\",\n  \"private\": true\n}\n";
    let consumer_lock = "lockfile-sentinel = 1\n";
    fs::write(ws_a.path().join("pyproject.toml"), consumer_pyproject).expect("write pyproject");
    fs::write(ws_a.path().join("package.json"), consumer_pkg_json).expect("write package.json");
    fs::write(ws_a.path().join("uv.lock"), consumer_lock).expect("write uv.lock");

    let env_a = make_env(ws_a.path(), VERSION);
    let env_b = make_env(ws_b.path(), VERSION);

    assert_eq!(
        run_cli(&env_a, &["playtest", "init"]).0,
        ExitStatus::Success
    );
    assert_eq!(
        run_cli(&env_b, &["playtest", "init"]).0,
        ExitStatus::Success
    );

    let (status_a, out_a, err_a) = run_cli(&env_a, &["playtest", "sync"]);
    assert_eq!(status_a, ExitStatus::Success, "stderr: {err_a}");
    assert!(err_a.is_empty());
    assert!(out_a.contains("Wiring commands (mirror path changed):"));
    assert!(out_a.contains("uv add --dev .playtest/python/playtest_py-"));
    assert!(out_a.contains("npm add --save-dev .playtest/typescript/archont561-playtest-"));

    let (status_b, _, _) = run_cli(&env_b, &["playtest", "sync"]);
    assert_eq!(status_b, ExitStatus::Success);

    // Consumer manifests and lockfiles are completely untouched (Spec 0013 §2.3, §3.4).
    assert_eq!(
        fs::read_to_string(ws_a.path().join("pyproject.toml")).expect("read pyproject"),
        consumer_pyproject
    );
    assert_eq!(
        fs::read_to_string(ws_a.path().join("package.json")).expect("read package.json"),
        consumer_pkg_json
    );
    assert_eq!(
        fs::read_to_string(ws_a.path().join("uv.lock")).expect("read uv.lock"),
        consumer_lock
    );

    // Byte-identical across independent runs/workspaces.
    let wheel_name = expected_mirror_filename("python", VERSION).expect("wheel name");
    let tgz_name = expected_mirror_filename("typescript", VERSION).expect("tgz name");
    let wheel_a = fs::read(ws_a.path().join(".playtest/python").join(&wheel_name)).expect("whl a");
    let wheel_b = fs::read(ws_b.path().join(".playtest/python").join(&wheel_name)).expect("whl b");
    let tgz_a = fs::read(ws_a.path().join(".playtest/typescript").join(&tgz_name)).expect("tgz a");
    let tgz_b = fs::read(ws_b.path().join(".playtest/typescript").join(&tgz_name)).expect("tgz b");

    assert_eq!(wheel_a, wheel_b);
    assert_eq!(tgz_a, tgz_b);
    assert_eq!(sha256_hex(&wheel_a).len(), 64);

    // Inspect PEP 427 wheel structure.
    let mut zip = ZipArchive::new(Cursor::new(&wheel_a)).expect("valid zip wheel");
    let mut init_py = String::new();
    zip.by_name("playtest/__init__.py")
        .expect("playtest/__init__.py in wheel")
        .read_to_string(&mut init_py)
        .expect("utf8 init");
    assert!(init_py.contains("VERSION"));

    let dist_info = format!("playtest_py-{}.dist-info", VERSION.replace('-', "_"));
    let mut metadata = String::new();
    zip.by_name(&format!("{dist_info}/METADATA"))
        .expect("METADATA in wheel")
        .read_to_string(&mut metadata)
        .expect("utf8 metadata");
    assert!(metadata.contains("Name: playtest-py"));
    assert!(metadata.contains(&format!("Version: {VERSION}")));
    assert!(!metadata.contains("typer"));

    let mut record = String::new();
    zip.by_name(&format!("{dist_info}/RECORD"))
        .expect("RECORD in wheel")
        .read_to_string(&mut record)
        .expect("utf8 record");
    assert!(record.contains("playtest/__init__.py,sha256="));
    assert!(record.contains(&format!("{dist_info}/RECORD,,")));

    // Inspect npm .tgz structure.
    let gz = GzDecoder::new(Cursor::new(&tgz_a));
    let mut tar = TarArchive::new(gz);
    let mut saw_pkg_json = false;
    let mut saw_dist_js = false;
    let mut saw_dts = false;
    for entry_res in tar.entries().expect("tar entries") {
        let mut entry = entry_res.expect("tar entry");
        let path = entry
            .path()
            .expect("entry path")
            .to_string_lossy()
            .into_owned();
        let mut body = String::new();
        entry.read_to_string(&mut body).expect("entry utf8");
        match path.as_str() {
            "package/package.json" => {
                saw_pkg_json = true;
                let parsed: JsonValue = serde_json::from_str(&body).expect("valid package.json");
                assert_eq!(parsed["name"], "@archont561/playtest");
                assert_eq!(parsed["version"], VERSION);
                assert!(parsed.get("bin").is_none());
                assert!(parsed.get("optionalDependencies").is_none());
                assert!(parsed.get("scripts").is_none());
                assert!(parsed.get("dependencies").is_none());
            }
            "package/dist/index.js" => saw_dist_js = true,
            "package/dist/index.d.ts" => saw_dts = true,
            _ => {}
        }
    }
    assert!(saw_pkg_json && saw_dist_js && saw_dts);

    // Second sync at the same version leaves files unchanged and omits wiring prompt.
    let (status_repeat, out_repeat, _) = run_cli(&env_a, &["playtest", "--format", "json", "sync"]);
    assert_eq!(status_repeat, ExitStatus::Success);
    let repeat_json: JsonValue = serde_json::from_str(&out_repeat).expect("valid sync json");
    assert_eq!(repeat_json["schema_version"], JSON_SCHEMA_VERSION);
    assert_eq!(repeat_json["wiring_commands"], serde_json::json!([]));
    assert_eq!(
        repeat_json["mirrors"][0]["status"],
        MirrorSyncStatus::Unchanged.as_str()
    );
}

#[test]
fn test_sync_replaces_previous_version_cleans_stale_files_and_check_detects_all_drift_modes() {
    let ws = TempDir::new().expect("tempdir");
    let env_v1 = make_env(ws.path(), "0.1.0");
    assert_eq!(
        run_cli(&env_v1, &["playtest", "init"]).0,
        ExitStatus::Success
    );

    // Before sync, `sync --check` reports missing mirrors and writes nothing.
    let (pre_status, _, pre_err) = run_cli(&env_v1, &["playtest", "sync", "--check"]);
    assert_eq!(pre_status, ExitStatus::EnvironmentFailure);
    assert!(pre_err.contains("[missing] .playtest/python/playtest_py-0.1.0-py3-none-any.whl"));
    assert!(pre_err.contains("playtest sync"));
    assert!(!ws.path().join(".playtest/python").exists());

    // Initial sync at 0.1.0 succeeds, and `sync --check` passes in both pretty and JSON modes.
    assert_eq!(
        run_cli(&env_v1, &["playtest", "sync"]).0,
        ExitStatus::Success
    );
    let (ok_status, ok_out, _) = run_cli(&env_v1, &["playtest", "sync", "--check"]);
    assert_eq!(ok_status, ExitStatus::Success);
    assert!(ok_out.contains("are up to date"));

    let (ok_json_status, ok_json_out, _) = run_cli(
        &env_v1,
        &["playtest", "--format", "json", "sync", "--check"],
    );
    assert_eq!(ok_json_status, ExitStatus::Success);
    let ok_parsed: JsonValue = serde_json::from_str(&ok_json_out).expect("json");
    assert_eq!(ok_parsed["ok"], true);

    // Wire consumer manifests to 0.1.0 mirrors, and corrupt/add stale files in .playtest/.
    fs::write(
        ws.path().join("package.json"),
        "{\n  \"devDependencies\": {\n    \"@archont561/playtest\": \"file:.playtest/typescript/archont561-playtest-0.1.0.tgz\"\n  }\n}\n",
    )
    .expect("write package.json");
    fs::write(
        ws.path().join("pyproject.toml"),
        "[tool.uv.sources]\nplaytest-py = { path = \".playtest/python/playtest_py-0.1.0-py3-none-any.whl\" }\n",
    )
    .expect("write pyproject.toml");

    let py_whl = ws
        .path()
        .join(".playtest/python/playtest_py-0.1.0-py3-none-any.whl");
    fs::write(&py_whl, b"corrupted-wheel").expect("corrupt wheel");
    fs::write(ws.path().join(".playtest/python/stray.txt"), b"extra").expect("stray file");
    fs::create_dir_all(ws.path().join(".playtest/obsolete_dir")).expect("obsolete dir");
    fs::write(
        ws.path()
            .join(WORKSPACE_DIR_NAME)
            .join(WORKSPACE_GITIGNORE_FILE),
        "# tampered\n",
    )
    .expect("tamper gitignore");

    let (drift_status, drift_json_raw, _) = run_cli(
        &env_v1,
        &["playtest", "--format", "json", "sync", "--check"],
    );
    assert_eq!(drift_status, ExitStatus::EnvironmentFailure);
    let drift_json: JsonValue = serde_json::from_str(&drift_json_raw).expect("drift json");
    assert_eq!(drift_json["ok"], false);
    let kinds: Vec<&str> = drift_json["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .filter_map(|item| item["kind"].as_str())
        .collect();
    assert!(kinds.contains(&MirrorDriftKind::Modified.as_str()));
    assert!(kinds.contains(&MirrorDriftKind::Stale.as_str()));

    // Running `sync` repairs modified wheel & gitignore and removes stray files/dirs.
    let (repair_status, repair_out, _) = run_cli(&env_v1, &["playtest", "sync"]);
    assert_eq!(repair_status, ExitStatus::Success);
    assert!(repair_out.contains("removed stale file: .playtest/python/stray.txt"));
    assert!(repair_out.contains("removed stale file: .playtest/obsolete_dir"));
    assert_eq!(
        run_cli(&env_v1, &["playtest", "sync", "--check"]).0,
        ExitStatus::Success
    );

    // Simulate upgrade to 0.2.0: update pin in playtest.toml, run `playtest sync`.
    let manifest_path = ws.path().join(WORKSPACE_MANIFEST_FILE);
    let updated_manifest = fs::read_to_string(&manifest_path)
        .expect("read manifest")
        .replace("\"0.1.0\"", "\"0.2.0\"");
    fs::write(&manifest_path, updated_manifest).expect("write 0.2.0 pin");

    let env_v2 = make_env(ws.path(), "0.2.0");
    let (up_status, up_out, _) = run_cli(&env_v2, &["playtest", "sync"]);
    assert_eq!(up_status, ExitStatus::Success);
    assert!(!py_whl.exists(), "0.1.0 wheel must be replaced");
    assert!(
        ws.path()
            .join(".playtest/python/playtest_py-0.2.0-py3-none-any.whl")
            .is_file()
    );
    assert!(
        ws.path()
            .join(".playtest/typescript/archont561-playtest-0.2.0.tgz")
            .is_file()
    );
    assert!(up_out.contains("uv add --dev .playtest/python/playtest_py-0.2.0-py3-none-any.whl"));

    // Spec 0013 §8 question 3: before repeating the wiring commands, `sync --check`
    // detects forgotten 0.1.0 mirror paths in `pyproject.toml` and `package.json`!
    let (stale_wiring_status, _, stale_wiring_err) =
        run_cli(&env_v2, &["playtest", "sync", "--check"]);
    assert_eq!(stale_wiring_status, ExitStatus::EnvironmentFailure);
    assert!(stale_wiring_err.contains("[stale_wiring] package.json"));
    assert!(stale_wiring_err.contains("[stale_wiring] pyproject.toml"));
    assert!(stale_wiring_err.contains("archont561-playtest-0.2.0.tgz"));
    assert!(stale_wiring_err.contains("playtest_py-0.2.0-py3-none-any.whl"));

    // Once consumer manifests are updated to 0.2.0, `sync --check` exits 0!
    fs::write(
        ws.path().join("package.json"),
        "{\n  \"devDependencies\": {\n    \"@archont561/playtest\": \"file:.playtest/typescript/archont561-playtest-0.2.0.tgz\"\n  }\n}\n",
    )
    .expect("update package.json");
    fs::write(
        ws.path().join("pyproject.toml"),
        "[tool.uv.sources]\nplaytest-py = { path = \".playtest/python/playtest_py-0.2.0-py3-none-any.whl\" }\n",
    )
    .expect("update pyproject.toml");
    assert_eq!(
        run_cli(&env_v2, &["playtest", "sync", "--check"]).0,
        ExitStatus::Success
    );
}

#[test]
fn test_sync_errors_on_missing_workspace_or_unsupported_mirror_language() {
    let outside = TempDir::new().expect("tempdir");
    let env = make_env(outside.path(), VERSION);
    let (missing_status, _, missing_err) = run_cli(&env, &["playtest", "sync"]);
    assert_eq!(missing_status, ExitStatus::EnvironmentFailure);
    assert!(missing_err.contains("no `playtest.toml` found"));

    fs::write(
        outside.path().join(WORKSPACE_MANIFEST_FILE),
        format!("schema = 1\nversion = {VERSION:?}\n\n[mirrors]\nlanguages = [\"rust\"]\n"),
    )
    .expect("write unsupported lang manifest");
    let (bad_lang_status, _, bad_lang_err) = run_cli(&env, &["playtest", "sync"]);
    assert_eq!(bad_lang_status, ExitStatus::EnvironmentFailure);
    assert!(bad_lang_err.contains("unsupported mirror language `rust`"));

    assert!(render_mirror("rust", VERSION).is_err());
    assert!(expected_mirror_filename("rust", VERSION).is_err());
    assert!(wiring_command_for("rust", ".playtest/rust").is_none());
}
