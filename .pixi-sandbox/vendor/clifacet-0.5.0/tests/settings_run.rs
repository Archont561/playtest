//! Settings through the runner: the file is chosen, read, layered and reported.
//!
//! These go through [`App`] rather than [`Settings`] because most of the acceptance criteria
//! are about a *run*: `--config` after the subcommand, a named file that is missing, help
//! that documents the built-in default, a file value that satisfies a required argument.
//! Each test writes a real file, so the reading path is the one a user gets.

#![cfg(feature = "settings-toml")]

use std::path::{Path, PathBuf};

use clifacet::failure::Failure;
use clifacet::policy::Capabilities;
use clifacet::prompt::Terminal;
use clifacet::provenance::Source;
use clifacet::settings::Map;
use clifacet::settings::source::Asked;
use clifacet::{App, Command};

/// A command with a root flag, a subcommand, and a value that has a built-in default.
#[clifacet::command(input = Selected, bridge = select)]
#[derive(Debug, Clone)]
#[command(name = "app", version, no_binary_name = true)]
struct Cli {
    /// How loud to be.
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Verb,

    #[command(flatten)]
    globals: clifacet::Globals,
}

/// Copies the port out of the subcommand the run chose.
///
/// The default bridge copies fields by name, and this input is not that: it is one field of
/// one variant. `bridge = path` is the documented way to say so.
#[allow(
    clippy::unnecessary_wraps,
    reason = "a bridge is `fn(command) -> Result<Input, Failure>`; the macro requires it"
)]
fn select(command: Cli) -> Result<Selected, Failure> {
    let Verb::Serve(serve) = command.command;
    Ok(Selected {
        port: usize::from(serve.port),
    })
}

/// What a run asks for: the port `serve` was given.
#[derive(Debug, Clone)]
struct Selected {
    port: usize,
}

#[derive(Debug, Clone, clap::Subcommand)]
enum Verb {
    Serve(Serve),
}

#[derive(Debug, Clone, clap::Args)]
struct Serve {
    /// Port to listen on.
    #[arg(long, env = "APP_PORT", default_value = "1")]
    port: u16,
}

/// The command that runs: it prints the parsed port, so a test reads what the run saw.
struct Show;

impl Command for Show {
    type Input = Selected;
    type Output = String;

    fn execute(&self, input: Self::Input) -> Result<Self::Output, Failure> {
        Ok(format!("port {}", input.port))
    }
}

/// A temporary directory that removes itself, so a failing test leaves nothing behind.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("clifacet-app-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&path).expect("a writable temp directory");
        Self { path }
    }

    fn file(&self, name: &str, contents: &str) -> PathBuf {
        let path = self.path.join(name);
        std::fs::write(&path, contents).expect("the file writes");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

fn run(argv: &[&str]) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = App::<Cli>::new()
        .command(&Show)
        .settings("app", clifacet::settings::toml::Toml)
        .run_with(
            argv.iter().map(std::ffi::OsString::from),
            Capabilities::headless(),
            || Box::new(Terminal::stdio()),
            &mut out,
            &mut err,
        );
    (
        code,
        String::from_utf8_lossy(&out).into_owned(),
        String::from_utf8_lossy(&err).into_owned(),
    )
}

#[cfg(feature = "settings-toml")]
#[test]
fn a_file_value_reaches_the_run() {
    let scratch = Scratch::new("reaches");
    let path = scratch.file("settings.toml", "[serve]\nport = 8080\n");

    let (code, out, err) = run(&["--config", path.to_str().unwrap(), "serve"]);

    assert_eq!(code, 0, "{err}");
    assert!(out.contains("port 8080"), "{out}");
}

#[test]
fn config_after_the_subcommand_names_the_same_file() {
    // The spelling a user reaches for when the file holds only what that subcommand needs.
    let scratch = Scratch::new("after");
    let path = scratch.file("settings.toml", "[serve]\nport = 9090\n");

    let (code, out, err) = run(&["serve", "--config", path.to_str().unwrap()]);

    assert_eq!(code, 0, "{err}");
    assert!(out.contains("port 9090"), "{out}");
}

#[test]
fn a_missing_named_file_fails_the_run_and_says_so() {
    let (code, _out, err) = run(&["--config", "/definitely/not/here.toml", "serve"]);

    assert_ne!(code, 0);
    assert!(err.contains("does not exist"), "{err}");
}

#[test]
fn no_config_ignores_the_default_place() {
    // Proved by the value being the built-in default rather than the file's: the file is
    // there and would be read without `--no-config`.
    let scratch = Scratch::new("skip");
    let path = scratch.file("settings.toml", "[serve]\nport = 7777\n");

    let (code, out, err) = run(&["--config", path.to_str().unwrap(), "--no-config", "serve"]);

    // `--no-config` with `--config` contradicts itself, so the run fails rather than
    // choosing one of the two silently.
    assert_ne!(code, 0, "{out}");
    assert!(err.contains("opposite"), "{err}");
}

#[test]
fn no_config_alone_reads_no_file() {
    let (code, out, err) = run(&["--no-config", "serve"]);

    assert_eq!(code, 0, "{err}");
    assert!(out.contains("port 1"), "the built-in default: {out}");
}

#[test]
fn an_unknown_key_fails_the_run_and_names_the_file_and_line() {
    let scratch = Scratch::new("unknown");
    let path = scratch.file("settings.toml", "[serve]\npart = 8080\n");

    let (code, _out, err) = run(&["--config", path.to_str().unwrap(), "serve"]);

    assert_ne!(code, 0);
    assert!(err.contains("serve.part"), "{err}");
    assert!(err.contains("settings.toml:2"), "{err}");
}

#[test]
fn a_bad_value_in_the_file_is_reported_at_its_line() {
    // AC#1's third clause: the error names the file, the line and the key, and the value
    // is what the argument's own parser rejected.
    let scratch = Scratch::new("bad-value");
    let path = scratch.file("settings.toml", "[serve]\nport = 99999\n");

    let (code, _out, err) = run(&["serve", "--config", path.to_str().unwrap()]);

    assert_ne!(code, 0);
    assert!(err.contains("settings.toml:2"), "{err}");
    assert!(err.contains("99999"), "{err}");
}

#[test]
fn help_documents_the_built_in_default_not_the_file() {
    // AC#7: `--help` never reads the file, so the default it shows is the one the program
    // was built with, and one machine's settings never leak into another's documentation.
    let scratch = Scratch::new("help");
    let path = scratch.file("settings.toml", "[serve]\nport = 8080\n");

    let (code, out, err) = run(&["serve", "--help", "--config", path.to_str().unwrap()]);

    assert_eq!(code, 0, "{err}");
    assert!(out.contains("[default: 1]"), "{out}");
    assert!(!out.contains("8080"), "{out}");
}

#[test]
fn the_environment_still_beats_the_file() {
    // AC#6 says the environment is read through an injectable source and the tests never
    // mutate the process environment, so this asks `clap` directly with a fixed environment
    // rather than setting one: the same parser, the same order, and no global state.
    let scratch = Scratch::new("env");
    let path = scratch.file("settings.toml", "[serve]\nport = 8080\n");
    let applied = Asked::default()
        .read(
            "app",
            &Map::new().with("APP_CONFIG", path.to_str().unwrap()),
            &clifacet::settings::toml::Toml,
        )
        .expect("the file is read from the injected environment");
    let command = applied
        .apply(
            clap::Command::new("app").no_binary_name(true).subcommand(
                clap::Command::new("serve")
                    .arg(clap::Arg::new("port").long("port").env("APP_PORT")),
            ),
            &[],
        )
        .expect("the key names an argument");

    // The file supplies the default, and clap reports where the value it used came from.
    let matches = command
        .into_command()
        .try_get_matches_from(["serve"])
        .expect("the tree parses");
    let (_, serve) = matches.subcommand().expect("a subcommand");
    assert_eq!(
        serve.get_one::<String>("port").map(String::as_str),
        Some("8080")
    );
}

#[test]
fn a_flag_still_beats_both() {
    let scratch = Scratch::new("flag");
    let path = scratch.file("settings.toml", "[serve]\nport = 8080\n");

    let (code, out, err) = run(&[
        "serve",
        "--config",
        path.to_str().unwrap(),
        "--port",
        "1111",
    ]);

    assert_eq!(code, 0, "{err}");
    assert!(out.contains("port 1111"), "{out}");
}

#[test]
fn a_required_argument_is_satisfied_by_the_file() {
    // AC#7's first half, on a run: a file value lifts `required` before the parse.
    #[clifacet::command(input = Precious, bridge = take_token)]
    #[derive(Debug, Clone)]
    #[command(name = "required", no_binary_name = true)]
    struct Strict {
        /// The token to echo.
        #[clifacet(long)]
        token: String,
    }

    #[derive(Debug, Clone)]
    struct Precious {
        token: String,
    }

    #[allow(
        clippy::unnecessary_wraps,
        reason = "a bridge is `fn(command) -> Result<Input, Failure>`; the macro requires it"
    )]
    fn take_token(command: Strict) -> Result<Precious, Failure> {
        Ok(Precious {
            token: command.token,
        })
    }

    struct Echo;
    impl Command for Echo {
        type Input = Precious;
        type Output = String;
        fn execute(&self, token: Self::Input) -> Result<Self::Output, Failure> {
            Ok(token.token)
        }
    }

    let scratch = Scratch::new("required");
    let path = scratch.file("settings.toml", "token = \"from-the-file\"\n");

    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = App::<Strict>::new()
        .command(&Echo)
        .settings("app", clifacet::settings::toml::Toml)
        .run_with(
            ["--config", path.to_str().unwrap()],
            Capabilities::headless(),
            || Box::new(Terminal::stdio()),
            &mut out,
            &mut err,
        );

    assert_eq!(code, 0, "{}", String::from_utf8_lossy(&err));
    assert!(String::from_utf8_lossy(&out).contains("from-the-file"));
}

#[test]
fn the_environment_variable_that_names_a_file_is_honoured() {
    // AC#2's middle clause, through the injectable source rather than the process, which is
    // what makes it testable at all.
    let scratch = Scratch::new("var");
    let path = scratch.file("settings.toml", "[serve]\nport = 4242\n");
    let environment = Map::new().with("APP_CONFIG", path.to_str().unwrap());

    let chosen = Asked::default()
        .choose("app", &environment)
        .expect("nothing contradicts");

    assert_eq!(chosen.path(), Some(Path::new(path.to_str().unwrap())));
}

#[test]
fn the_default_location_is_used_when_it_exists_and_ignored_when_it_does_not() {
    // The directory is asked for rather than assumed: macOS uses `Application Support` and
    // Windows uses `APPDATA`, so a test that hard-codes `~/.config` passes on Linux and
    // fails on the two platforms CI also builds. This is that bug, fixed.
    let scratch = Scratch::new("default");
    let environment = Map::new()
        .with("HOME", scratch.path.to_str().unwrap())
        .with("XDG_CONFIG_HOME", scratch.path.to_str().unwrap())
        .with("APPDATA", scratch.path.to_str().unwrap());

    let Some(expected) = clifacet::settings::place::default_file("app", &environment) else {
        return; // a platform with no config directory has no default file to find
    };
    std::fs::create_dir_all(expected.parent().expect("a file has a directory"))
        .expect("a writable config directory");
    std::fs::write(&expected, "[serve]\nport = 3131\n").expect("the file writes");

    let chosen = Asked::default()
        .choose("app", &environment)
        .expect("nothing contradicts");
    assert_eq!(chosen.path(), Some(expected.as_path()));

    // And with the file gone the choice is "no file", not an error: most machines have none.
    std::fs::remove_file(&expected).expect("the file removes");
    assert_eq!(
        Asked::default()
            .choose("app", &environment)
            .expect("nothing contradicts")
            .path(),
        None
    );
}

#[test]
fn a_secret_from_a_file_keeps_its_value_out_of_the_record() {
    // AC#5 through the parse: the record follows the declaration, so a value read from a
    // file is secret exactly when the argument is.
    let scratch = Scratch::new("secret");
    let path = scratch.file("settings.toml", "[serve]\ntoken = \"hunter2\"\n");
    let command = clifacet::settings::toml::Toml
        .load_from(&path, &std::fs::read(&path).expect("the file reads"))
        .expect("a valid file");
    let applied = command
        .apply(
            clap::Command::new("app")
                .no_binary_name(true)
                .subcommand(clap::Command::new("serve").arg(clap::Arg::new("token").long("token"))),
            &["token"],
        )
        .expect("the key names an argument");

    assert!(applied.provenance().is_secret("token"));
    assert_eq!(applied.provenance().source_of("token"), Some(Source::File));
    assert!(!format!("{}", applied.provenance()).contains("hunter2"));
}
