use std::fs;
use std::io;
use std::path::Path;

use assert_cmd::Command as AssertCommand;
use playtest_assets::SdkLanguage;
use playtest_cli::{ExitStatus, OutputFormat, RunEnvironment, run_with_env};
use playtest_core::{
    WORKSPACE_DIR_NAME, WORKSPACE_GITIGNORE_FILE, WORKSPACE_MANIFEST_FILE, WorkspaceConfig,
    require_workspace,
};
use rstest::rstest;
use serde_json::Value;
use tempfile::TempDir;

struct Invocation {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

fn invoke(argv: &[&str], env: &RunEnvironment) -> Invocation {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let status = run_with_env(argv, env, &mut out, &mut err);
    Invocation {
        status,
        stdout: String::from_utf8(out).expect("stdout utf-8"),
        stderr: String::from_utf8(err).expect("stderr utf-8"),
    }
}

#[rstest]
fn test_version_matches_workspace_crates() {
    assert_eq!(playtest_cli::VERSION, "0.1.0");
    assert_eq!(playtest_cli::VERSION, playtest_core::VERSION);
    assert_eq!(playtest_cli::VERSION, playtest_assets::VERSION);
}

#[rstest]
fn test_binary_playtest_version_outside_workspace_prints_cli_binary_and_all_sdks() {
    let temp = TempDir::new().expect("tempdir");

    let output = AssertCommand::new(assert_cmd::cargo::cargo_bin!("playtest"))
        .current_dir(temp.path())
        .arg("version")
        .output()
        .expect("run playtest version");

    assert_eq!(output.status.code(), Some(0));
    let stdout = String::from_utf8(output.stdout).expect("stdout utf-8");
    let stderr = String::from_utf8(output.stderr).expect("stderr utf-8");
    assert!(stderr.is_empty(), "unexpected stderr: {stderr}");

    assert!(stdout.contains(&format!("playtest {}", playtest_cli::VERSION)));
    assert!(stdout.contains("binary: "));
    assert!(stdout.contains("playtest"));
    assert!(stdout.contains("workspace: none"));
    assert!(stdout.contains(&format!("rust sdk: {}", playtest_core::VERSION)));

    for lang in SdkLanguage::ALL {
        let expected_line = format!("  {}: {}", lang.as_str(), playtest_assets::VERSION);
        assert!(
            stdout.contains(&expected_line),
            "missing embedded SDK line `{expected_line}` in:\n{stdout}"
        );
    }
    assert!(!stdout.to_ascii_lowercase().contains("clifacet"));
}

#[rstest]
fn test_version_json_output_carries_schema_version_and_all_embedded_sdks() {
    let temp = TempDir::new().expect("tempdir");
    let env = RunEnvironment::new(temp.path(), "/usr/local/bin/playtest");

    let res = invoke(&["playtest", "version", "--format", "json"], &env);
    assert_eq!(res.status, ExitStatus::Success);
    assert!(res.stderr.is_empty());

    let parsed: Value = serde_json::from_str(&res.stdout).expect("valid JSON on stdout");
    assert_eq!(parsed["schema_version"], 1);
    assert_eq!(parsed["cli_version"], playtest_cli::VERSION);
    assert_eq!(parsed["binary_path"], "/usr/local/bin/playtest");
    assert_eq!(parsed["rust_sdk_version"], playtest_core::VERSION);
    assert!(parsed["workspace"].is_null());

    for lang in SdkLanguage::ALL {
        assert_eq!(
            parsed["embedded_sdks"][lang.as_str()],
            playtest_assets::VERSION
        );
    }
}

#[rstest]
fn test_first_and_repeated_init_are_idempotent_and_touch_no_other_files() {
    let temp = TempDir::new().expect("tempdir");
    let pyproject = temp.path().join("pyproject.toml");
    let package_json = temp.path().join("package.json");
    let lockfile = temp.path().join("uv.lock");
    fs::write(&pyproject, "[project]\nname = \"demo\"\n").expect("write pyproject");
    fs::write(&package_json, "{\"name\":\"demo\"}\n").expect("write package.json");
    fs::write(&lockfile, "version = 1\n").expect("write lockfile");

    // 1. First `playtest init` via the compiled `playtest` binary
    let first = AssertCommand::new(assert_cmd::cargo::cargo_bin!("playtest"))
        .current_dir(temp.path())
        .arg("init")
        .output()
        .expect("first playtest init");
    assert_eq!(first.status.code(), Some(0));
    let first_stdout = String::from_utf8(first.stdout).expect("utf-8");
    assert!(first_stdout.contains("playtest sync"));
    assert!(first_stdout.contains("uv add --dev"));
    assert!(first_stdout.contains("npm add --save-dev"));

    let manifest_path = temp.path().join(WORKSPACE_MANIFEST_FILE);
    let gitignore_path = temp
        .path()
        .join(WORKSPACE_DIR_NAME)
        .join(WORKSPACE_GITIGNORE_FILE);
    assert!(manifest_path.is_file());
    assert!(gitignore_path.is_file());

    let manifest_text = fs::read_to_string(&manifest_path).expect("read playtest.toml");
    let parsed_cfg = WorkspaceConfig::from_toml_str(&manifest_text, &manifest_path)
        .expect("generated playtest.toml parses");
    assert_eq!(parsed_cfg.schema, 1);
    assert_eq!(parsed_cfg.version, playtest_cli::VERSION);
    assert_eq!(parsed_cfg.mirrors.languages, vec!["python", "typescript"]);

    let gitignore_text = fs::read_to_string(&gitignore_path).expect("read .gitignore");
    assert!(gitignore_text.contains("!*.whl"));
    assert!(gitignore_text.contains("!*.tgz"));

    // Consumer files were not edited, and no extra files were created
    assert_eq!(
        fs::read_to_string(&pyproject).expect("pyproject"),
        "[project]\nname = \"demo\"\n"
    );
    assert_eq!(
        fs::read_to_string(&package_json).expect("package.json"),
        "{\"name\":\"demo\"}\n"
    );
    assert_eq!(
        fs::read_to_string(&lockfile).expect("uv.lock"),
        "version = 1\n"
    );

    // 2. Customize `playtest.toml` (keeping matching version pin) and run `playtest init` again
    let edited_manifest = format!(
        "# Custom consumer comment\nschema = 1\nversion = {:?}\n\n[mirrors]\nlanguages = [\"python\"]\n",
        playtest_cli::VERSION
    );
    fs::write(&manifest_path, &edited_manifest).expect("write edited manifest");

    let second = AssertCommand::new(assert_cmd::cargo::cargo_bin!("playtest"))
        .current_dir(temp.path())
        .arg("init")
        .output()
        .expect("second playtest init");
    assert_eq!(second.status.code(), Some(0));
    let second_stdout = String::from_utf8(second.stdout).expect("utf-8");
    assert!(second_stdout.contains("already initialized"));

    assert_eq!(
        fs::read_to_string(&manifest_path).expect("manifest preserved"),
        edited_manifest,
        "repeated `playtest init` must never overwrite an edited playtest.toml"
    );
    assert_eq!(
        fs::read_to_string(&gitignore_path).expect("gitignore preserved"),
        gitignore_text
    );
}

#[rstest]
fn test_matching_pin_allows_commands_from_subdirectory() {
    let temp = TempDir::new().expect("tempdir");
    let root_env = RunEnvironment::new(temp.path(), "/opt/playtest/bin/playtest");
    assert_eq!(
        invoke(&["playtest", "init"], &root_env).status,
        ExitStatus::Success
    );

    let sub = temp.path().join("apps/backend");
    fs::create_dir_all(&sub).expect("create subdir");
    let sub_env = RunEnvironment::new(&sub, "/opt/playtest/bin/playtest");

    let init_res = invoke(&["playtest", "init", "--format", "json"], &sub_env);
    assert_eq!(init_res.status, ExitStatus::Success);
    assert!(init_res.stderr.is_empty());
    let init_json: Value = serde_json::from_str(&init_res.stdout).expect("init json");
    assert_eq!(init_json["created_manifest"], false);
    assert_eq!(init_json["created_gitignore"], false);
    assert!(!sub.join(WORKSPACE_MANIFEST_FILE).exists());

    let ver_res = invoke(&["playtest", "--format", "json", "version"], &sub_env);
    assert_eq!(ver_res.status, ExitStatus::Success);
    assert!(ver_res.stderr.is_empty());
    let ver_json: Value = serde_json::from_str(&ver_res.stdout).expect("version json");
    assert_eq!(
        ver_json["workspace"]["pinned_version"],
        playtest_cli::VERSION
    );
    assert_eq!(ver_json["workspace"]["matches_cli"], true);
}

#[rstest]
fn test_mismatched_pin_exits_3_for_commands_and_warns_on_version() {
    let temp = TempDir::new().expect("tempdir");
    let manifest_path = temp.path().join(WORKSPACE_MANIFEST_FILE);
    fs::write(
        &manifest_path,
        "schema = 1\nversion = \"0.2.0\"\n\n[mirrors]\nlanguages = [\"python\"]\n",
    )
    .expect("write pinned playtest.toml");

    // Non-`version` command (`init`) via real binary must exit 3 and name both versions,
    // the running binary path, and both fixes (Spec 0013 §6.2).
    let init_out = AssertCommand::new(assert_cmd::cargo::cargo_bin!("playtest"))
        .current_dir(temp.path())
        .arg("init")
        .output()
        .expect("run playtest init on mismatched workspace");

    assert_eq!(init_out.status.code(), Some(3));
    assert!(
        init_out.stdout.is_empty(),
        "stdout must stay empty on pin failure"
    );
    let init_stderr = String::from_utf8(init_out.stderr).expect("stderr utf-8");
    assert!(
        init_stderr.contains("0.2.0"),
        "missing pinned version: {init_stderr}"
    );
    assert!(
        init_stderr.contains(playtest_cli::VERSION),
        "missing running version: {init_stderr}"
    );
    assert!(
        init_stderr.contains("playtest"),
        "missing binary path: {init_stderr}"
    );
    assert!(
        init_stderr.contains("install playtest 0.2.0"),
        "missing fix 1 (install pinned version): {init_stderr}"
    );
    assert!(
        init_stderr.contains("playtest sync"),
        "missing fix 2 (update pin and run playtest sync): {init_stderr}"
    );
    assert!(
        !temp.path().join(WORKSPACE_DIR_NAME).exists(),
        "failed pin check must write nothing"
    );

    // `playtest version` is exempt (Spec 0013 §6.3): exits 0, prints version & binary path
    // on stdout, and warns about the mismatch on stderr.
    let env = RunEnvironment::new(temp.path(), "/custom/path/to/playtest");
    let ver_res = invoke(&["playtest", "version"], &env);
    assert_eq!(ver_res.status, ExitStatus::Success);
    assert!(
        ver_res
            .stdout
            .contains(&format!("playtest {}", playtest_cli::VERSION))
    );
    assert!(ver_res.stdout.contains("binary: /custom/path/to/playtest"));
    assert!(ver_res.stdout.contains("pinned 0.2.0, mismatch"));
    assert!(ver_res.stderr.contains("warning:"));
    assert!(ver_res.stderr.contains("0.2.0"));
    assert!(ver_res.stderr.contains("/custom/path/to/playtest"));
    assert!(ver_res.stderr.contains("playtest sync"));
}

#[rstest]
fn test_missing_playtest_toml_and_malformed_manifest_handling() {
    let temp = TempDir::new().expect("tempdir");
    let env = RunEnvironment::new(temp.path(), "/usr/bin/playtest");

    // Missing `playtest.toml`: workspace-requiring check returns EnvironmentFailure-grade error
    let missing = require_workspace(&env.cwd, &env.cli_version, &env.binary_path)
        .expect_err("missing playtest.toml must fail require_workspace");
    assert!(missing.to_string().contains("no `playtest.toml` found"));
    assert!(missing.to_string().contains("playtest init"));

    // Malformed `playtest.toml`: non-version command exits 3; `version` warns and still exits 0
    fs::write(temp.path().join(WORKSPACE_MANIFEST_FILE), "not = [valid\n")
        .expect("write broken playtest.toml");
    let bad_init = invoke(&["playtest", "init"], &env);
    assert_eq!(bad_init.status, ExitStatus::EnvironmentFailure);
    assert!(bad_init.stdout.is_empty());
    assert!(bad_init.stderr.contains("invalid `playtest.toml`"));

    let bad_ver = invoke(&["playtest", "version"], &env);
    assert_eq!(bad_ver.status, ExitStatus::Success);
    assert!(bad_ver.stdout.contains("binary: /usr/bin/playtest"));
    assert!(bad_ver.stderr.contains("warning:"));
}

#[rstest]
fn test_init_lang_selection_and_usage_error_exit_codes() {
    let temp = TempDir::new().expect("tempdir");
    let env = RunEnvironment::new(temp.path(), "/usr/bin/playtest");

    // Invalid `--lang` -> exit 2
    let bad_lang = invoke(&["playtest", "init", "--lang", "fortran"], &env);
    assert_eq!(bad_lang.status, ExitStatus::UsageError);
    assert!(bad_lang.stdout.is_empty());
    assert!(bad_lang.stderr.contains("fortran"));

    // Valid `--lang py --lang python` -> deduplicates to `["python"]`
    let ok_lang = invoke(
        &["playtest", "init", "--lang", "py", "--lang", "python"],
        &env,
    );
    assert_eq!(ok_lang.status, ExitStatus::Success);
    let manifest_text =
        fs::read_to_string(temp.path().join(WORKSPACE_MANIFEST_FILE)).expect("read manifest");
    assert!(manifest_text.contains("languages = [\"python\"]"));

    // Usage errors (exit 2) vs help / --version (exit 0)
    let help = invoke(&["playtest", "--help"], &env);
    assert_eq!(help.status, ExitStatus::Success);
    assert!(help.stdout.contains("version"));
    assert!(help.stdout.contains("init"));
    assert!(!help.stdout.to_ascii_lowercase().contains("clifacet"));

    let short_ver = invoke(&["playtest", "--version"], &env);
    assert_eq!(short_ver.status, ExitStatus::Success);
    assert_eq!(
        short_ver.stdout.trim(),
        format!("playtest {}", playtest_cli::VERSION)
    );

    for bad_argv in [
        &["playtest"][..],
        &["playtest", "unknown-cmd"][..],
        &["playtest", "--unknown-flag"][..],
        &["playtest", "version", "--format", "yaml"][..],
    ] {
        let res = invoke(bad_argv, &env);
        assert_eq!(
            res.status,
            ExitStatus::UsageError,
            "expected exit 2 for {bad_argv:?}, got {:?} (stderr: {})",
            res.status,
            res.stderr
        );
        assert!(res.stdout.is_empty());
    }
}

#[rstest]
fn test_closed_stdout_returns_environment_failure_and_public_api_hides_clifacet() {
    struct BrokenPipeWriter;

    impl io::Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
    }

    let temp = TempDir::new().expect("tempdir");
    let env = RunEnvironment::new(temp.path(), "/usr/bin/playtest")
        .with_cli_version(playtest_cli::VERSION);
    let mut err = Vec::new();
    let status = run_with_env(
        ["playtest", "version"],
        &env,
        &mut BrokenPipeWriter,
        &mut err,
    );
    assert_eq!(status, ExitStatus::EnvironmentFailure);

    let help_status = run_with_env(
        ["playtest", "--help"],
        &env,
        &mut BrokenPipeWriter,
        &mut err,
    );
    assert_eq!(help_status, ExitStatus::EnvironmentFailure);

    // If `<cwd>/playtest.toml` is a directory, `init` cannot write the file -> EnvironmentFailure (3).
    fs::create_dir_all(temp.path().join(WORKSPACE_MANIFEST_FILE))
        .expect("create directory blocking playtest.toml");
    let blocked_init = invoke(&["playtest", "init"], &env);
    assert_eq!(blocked_init.status, ExitStatus::EnvironmentFailure);
    assert!(
        blocked_init
            .stderr
            .contains("failed to initialize workspace")
    );

    assert_eq!(OutputFormat::Pretty.as_str(), "pretty");
    assert_eq!(OutputFormat::Json.as_str(), "json");

    // Spec 0012 §1: neither playtest-core nor public playtest-cli modules expose clifacet types.
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cli_lib = fs::read_to_string(manifest_dir.join("src/lib.rs")).expect("read cli lib.rs");
    let cli_out =
        fs::read_to_string(manifest_dir.join("src/output.rs")).expect("read cli output.rs");
    for line in cli_lib.lines().chain(cli_out.lines()) {
        let trimmed = line.trim();
        if !trimmed.starts_with("//") {
            assert!(
                !trimmed.contains("clifacet"),
                "public CLI module leaked `clifacet`: {trimmed}"
            );
        }
    }
}
