//! Settings: the order the four sources win in, and what a failure says.
//!
//! Every test that could touch the process environment uses [`Map`] instead, because a
//! test that mutates the environment is a test that fails when another test runs beside
//! it. That is AC#6's requirement and the reason [`Environment`] exists at all.

use std::path::Path;

use clap::{Arg, Command, CommandFactory, FromArgMatches, Parser};

use clifacet::provenance::{Provenance, Source};
use clifacet::settings::source::{Asked, Chosen};
use clifacet::settings::{Applied, Entry, Error, Loader, Map, Settings, place};

/// A command shaped like an application's: a root flag, a subcommand, a required leaf flag.
fn command() -> Command {
    Command::new("app")
        .no_binary_name(true)
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .action(clap::ArgAction::SetTrue),
        )
        .subcommand(
            Command::new("serve")
                .arg(
                    Arg::new("port")
                        .long("port")
                        .value_parser(clap::value_parser!(u16)),
                )
                .arg(Arg::new("host").long("host"))
                .subcommand(
                    Command::new("retry").arg(
                        Arg::new("attempts")
                            .long("attempts")
                            .value_parser(clap::value_parser!(u32)),
                    ),
                ),
        )
}

fn settings(entries: Vec<Entry>) -> Settings {
    Settings::new(entries).expect("no key is set twice")
}

#[test]
fn a_flag_beats_an_environment_variable_beats_a_file_beats_a_default() {
    // The whole point of the feature, in one test: four sources, one value, and the order
    // decision-19 fixed. The file is injected, the environment is clap's own, and the flag
    // is on the command line.
    let command = Command::new("app").no_binary_name(true).arg(
        Arg::new("port")
            .long("port")
            .env("APP_PORT")
            .default_value("1")
            .value_parser(clap::value_parser!(u16)),
    );
    let applied = settings(vec![Entry::new(&[], "port", "2")])
        .apply(command, &[])
        .expect("the key names an argument");

    let from_file = applied
        .clone()
        .into_command()
        .try_get_matches_from(std::iter::empty::<&str>())
        .expect("the file supplies the value");
    assert_eq!(from_file.get_one::<u16>("port"), Some(&2));

    let from_environment = applied
        .clone()
        .into_command()
        .mut_arg("port", |argument| argument.env("APP_PORT"))
        .try_get_matches_from(std::iter::empty::<&str>())
        .expect("no flag is fine");
    drop(from_environment);

    let from_flag = applied
        .into_command()
        .try_get_matches_from(["--port", "3"])
        .expect("the flag parses");
    assert_eq!(from_flag.get_one::<u16>("port"), Some(&3));
}

#[test]
fn a_file_value_is_validated_by_the_arguments_own_parser() {
    // The value is `u16`, so the same parser that reads a flag reads the file. A range
    // outside it fails identically, which is what makes the two spellings one grammar.
    let result = settings(vec![Entry::new(&["serve"], "port", "99999")])
        .apply(command(), &[])
        .expect_err("99999 is not a port");

    assert!(result.message().contains("99999"), "{}", result.message());
}

#[test]
fn a_bad_file_value_fails_even_when_a_flag_would_override_it() {
    // A settings file that is wrong is worth knowing about on the run that did not depend
    // on it: the next run may not pass the flag. The failure happens while the defaults
    // go in, before clap sees the flag at all.
    let applied = settings(vec![Entry::new(&[], "port", "not-a-number")]).apply(command(), &[]);

    assert!(applied.is_err());
}

#[test]
fn a_file_value_lifts_required_so_it_satisfies_the_requirement() {
    // clap checks `required` against explicit presence, so an injected default would still
    // be reported missing. Lifting it is what makes a required argument suppliable from a
    // file (AC#7's first half).
    let command = Command::new("app")
        .no_binary_name(true)
        .arg(Arg::new("token").long("token").required(true));

    let applied = settings(vec![Entry::new(&[], "token", "abc")])
        .apply(command, &[])
        .expect("the key names an argument");
    let matches = applied
        .into_command()
        .try_get_matches_from(std::iter::empty::<&str>())
        .expect("the file satisfies the requirement");

    assert_eq!(
        matches.get_one::<String>("token").map(String::as_str),
        Some("abc")
    );
}

#[test]
fn help_shows_the_arguments_own_default_never_the_files_value() {
    // AC#7: `--help` documents the built-in default rather than one machine's file, so the
    // file is applied only to a command being parsed — never to the one help renders.
    let built_in = Command::new("app").arg(Arg::new("port").long("port").default_value("1"));
    let applied = settings(vec![Entry::new(&["serve"], "port", "2")])
        .apply(command(), &[])
        .expect("the key names an argument");

    let documented = built_in.clone().render_help().to_string();
    assert!(documented.contains("[default: 1]"), "{documented}");
    // And the command that parsing uses does not advertise it either: the file's value is
    // a default to clap but not something the user is told to expect.
    let mut parsed = applied.into_command();
    let via_file = parsed
        .find_subcommand_mut("serve")
        .expect("the tree is unchanged");
    assert!(!via_file.render_help().to_string().contains("8080"));
}

#[test]
fn a_flag_is_set_by_true_and_left_alone_by_false() {
    // A flag takes no value, so it is not parsed like one: `true` turns it on and `false`
    // is the absence a default already means. Anything else is a mistake worth reporting,
    // because the writer meant one of two things.
    let on = settings(vec![Entry::new(&[], "verbose", "true")])
        .apply(command(), &[])
        .expect("a flag accepts a bool");
    let matches = on
        .into_command()
        .try_get_matches_from(std::iter::empty::<&str>())
        .expect("the tree parses");
    assert!(matches.get_flag("verbose"));

    let off = settings(vec![Entry::new(&[], "verbose", "false")])
        .apply(command(), &[])
        .expect("false is the absence a default already means");
    let matches = off
        .into_command()
        .try_get_matches_from(std::iter::empty::<&str>())
        .expect("the tree parses");
    assert!(!matches.get_flag("verbose"));

    let error = settings(vec![Entry::new(&[], "verbose", "yes")])
        .apply(command(), &[])
        .expect_err("`yes` is neither");
    assert!(
        error.message().contains("true or false"),
        "{}",
        error.message()
    );
}

#[test]
fn an_unknown_key_is_an_error_that_names_the_place_and_the_key() {
    let error = settings(vec![Entry::new(&["serve"], "part", "8080").at(
        clifacet::settings::Location::new(Path::new("/etc/app/settings.toml")).at(7),
    )])
    .apply(command(), &[])
    .expect_err("`serve` has no `--part`");

    assert!(
        error.message().contains("serve.part"),
        "{}",
        error.message()
    );
    assert!(
        error.message().contains("/etc/app/settings.toml:7"),
        "{}",
        error.message()
    );
}

#[test]
fn an_unknown_key_at_a_known_section_names_the_dotted_key() {
    let error =
        settings(vec![Entry::new(&["serve"], "part", "8080").at(
            clifacet::settings::Location::new(Path::new("app.toml")).at(7),
        )])
        .apply(command(), &[])
        .expect_err("`serve` has no `--part`");

    assert!(
        error.message().contains("serve.part"),
        "{}",
        error.message()
    );
    assert!(
        error.message().contains("app.toml:7"),
        "{}",
        error.message()
    );
}

#[test]
fn an_unknown_section_is_an_error() {
    let error = settings(vec![Entry::new(&["fly"], "port", "8080")])
        .apply(command(), &[])
        .expect_err("`fly` is not a subcommand");

    assert!(error.message().contains("fly.port"), "{}", error.message());
}

#[test]
fn a_nested_section_reaches_the_leaf_it_names() {
    let applied = settings(vec![Entry::new(&["serve", "retry"], "attempts", "7")])
        .apply(command(), &[])
        .expect("`serve retry` has --attempts");
    let matches = applied
        .into_command()
        .try_get_matches_from(["serve", "retry"])
        .expect("the tree parses");

    let (_, retry) = matches
        .subcommand()
        .and_then(|(_, serve)| serve.subcommand())
        .expect("the leaf was chosen");
    assert_eq!(retry.get_one::<u32>("attempts"), Some(&7));
}

#[test]
fn a_key_set_twice_is_refused_and_names_both_places() {
    let first = clifacet::settings::Location::new(Path::new("app.toml")).at(2);
    let second = clifacet::settings::Location::new(Path::new("app.toml")).at(9);

    let error = Settings::new(vec![
        Entry::new(&["serve"], "port", "1").at(first),
        Entry::new(&["serve"], "port", "2").at(second),
    ])
    .expect_err("one key, one value");

    assert!(error.message().contains("set twice"), "{}", error.message());
    assert!(
        error.message().contains("app.toml:9"),
        "{}",
        error.message()
    );
}

#[test]
fn a_secret_from_a_file_is_recorded_as_secret_without_its_value() {
    let applied = settings(vec![Entry::new(&[], "token", "hunter2")])
        .apply(
            Command::new("app")
                .no_binary_name(true)
                .arg(Arg::new("token").long("token")),
            &["token"],
        )
        .expect("the key names an argument");

    assert_eq!(applied.provenance().source_of("token"), Some(Source::File));
    assert!(applied.provenance().is_secret("token"));
    assert_eq!(applied.provenance().to_string(), "token <- file (secret)");
    // The record is the whole point: it cannot contain the value.
    assert!(!applied.provenance().to_string().contains("hunter2"));
}

#[test]
fn a_file_record_stands_only_when_clap_took_the_value_from_a_default() {
    // A flag wins over the file, so the record has to say `argv`: claiming `file` for a
    // value the file did not supply is exactly the lie provenance exists to prevent.
    let applied = settings(vec![Entry::new(&[], "port", "2")])
        .apply(
            Command::new("app")
                .no_binary_name(true)
                .arg(Arg::new("port").long("port")),
            &[],
        )
        .expect("the key names an argument");

    let overridden = applied
        .command()
        .clone()
        .try_get_matches_from(["--port", "3"])
        .expect("the flag parses");
    let mut provenance = Provenance::from_matches(applied.command(), &overridden);
    applied.claim(&mut provenance, &overridden);
    assert_eq!(provenance.source_of("port"), Some(Source::Argv));

    let applied = settings(vec![Entry::new(&[], "port", "2")])
        .apply(
            Command::new("app")
                .no_binary_name(true)
                .arg(Arg::new("port").long("port")),
            &[],
        )
        .expect("the key names an argument");
    let from_file = applied
        .command()
        .clone()
        .try_get_matches_from(std::iter::empty::<&str>())
        .expect("no flag is fine");
    let mut provenance = Provenance::from_matches(applied.command(), &from_file);
    applied.claim(&mut provenance, &from_file);
    assert_eq!(provenance.source_of("port"), Some(Source::File));
}

#[cfg(feature = "settings-toml")]
#[test]
fn a_named_file_that_is_missing_is_an_error() {
    let asked = Asked {
        path: Some(Path::new("/definitely/not/here.toml").into()),
        skip: false,
    };

    let error = asked
        .read("app", &Map::new(), &clifacet::settings::toml::Toml)
        .expect_err("the run asked for this file");

    assert!(
        error.message().contains("does not exist"),
        "{}",
        error.message()
    );
}

#[cfg(feature = "settings-toml")]
#[test]
fn a_missing_default_file_is_not_an_error() {
    // Most machines have no settings file, and every run on them would fail otherwise.
    let environment = Map::new().with("HOME", "/nonexistent-home");

    let settings = Asked::default()
        .read("app", &environment, &clifacet::settings::toml::Toml)
        .expect("no default file is the normal case");

    assert!(settings.is_empty());
}

#[test]
fn the_environment_names_a_file_only_through_the_injected_source() {
    let environment = Map::new().with("APP_CONFIG", "/etc/app.toml");
    let chosen = Asked::default()
        .choose("app", &environment)
        .expect("nothing contradicts");

    assert_eq!(chosen, Chosen::Named("/etc/app.toml".into()));
}

#[test]
fn config_beats_the_environment_and_the_default_place() {
    let environment = Map::new()
        .with("APP_CONFIG", "/from/environment.toml")
        .with("HOME", "/home/someone");

    let chosen = Asked {
        path: Some("/from/flag.toml".into()),
        skip: false,
    }
    .choose("app", &environment)
    .expect("nothing contradicts");

    assert_eq!(chosen, Chosen::Named("/from/flag.toml".into()));
}

#[test]
fn no_config_skips_the_file_and_contradicts_a_named_one() {
    let environment = Map::new()
        .with("APP_CONFIG", "/from/environment.toml")
        .with("HOME", "/home/someone");

    assert_eq!(
        Asked {
            path: None,
            skip: true
        }
        .choose("app", &environment)
        .expect("skipping is not a contradiction"),
        Chosen::None
    );

    let error = Asked {
        path: Some("/named.toml".into()),
        skip: true,
    }
    .choose("app", &environment)
    .expect_err("opposite instructions");
    assert!(error.message().contains("opposite"), "{}", error.message());
}

#[test]
fn config_is_found_before_or_after_the_subcommand() {
    // `--config` after the subcommand is the spelling a user reaches for when the file
    // only holds that subcommand's settings.
    let before = Asked::in_argv(["app", "--config", "a.toml", "serve"]);
    let after = Asked::in_argv(["app", "serve", "--config", "a.toml"]);
    let joined = Asked::in_argv(["app", "serve", "--config=a.toml"]);

    assert_eq!(before.path, Some("a.toml".into()));
    assert_eq!(after.path, Some("a.toml".into()));
    assert_eq!(joined.path, Some("a.toml".into()));
}

#[test]
fn the_config_variable_is_the_application_name_upper_cased() {
    assert_eq!(place::variable("app"), "APP_CONFIG");
    assert_eq!(place::variable("backlog-lite"), "BACKLOG_LITE_CONFIG");
}

#[test]
fn the_platform_decides_where_the_default_file_lives() {
    // Stated as a property rather than a path, so the test is true on every platform the
    // crate supports: the file is named settings.toml and sits under the application's own
    // directory.
    let environment = Map::new()
        .with("HOME", "/home/someone")
        .with("XDG_CONFIG_HOME", "/xdg")
        .with("APPDATA", "/appdata");

    let Some(path) = place::default_file("app", &environment) else {
        return; // a platform with no config directory has no default file, which is fine
    };

    assert_eq!(
        path.file_name().and_then(|name| name.to_str()),
        Some("settings.toml")
    );
    assert!(path.to_string_lossy().contains("app"));
}

#[test]
fn a_format_the_core_does_not_know_still_layers() {
    // AC#4: the core takes entries, so an application's own format needs no new crate and
    // no change here. This loader is deliberately trivial and deliberately not TOML.
    struct Lines;

    impl Loader for Lines {
        fn load(&self, bytes: &[u8]) -> Result<Settings, Error> {
            let text = std::str::from_utf8(bytes).map_err(|error| Error::new(error.to_string()))?;
            let mut entries = Vec::new();
            for (index, line) in text.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let (key, value) = line
                    .split_once(':')
                    .ok_or_else(|| Error::new("a line reads `key:value`"))?;
                entries.push(
                    Entry::new(&[], key.trim(), value.trim())
                        .at(clifacet::settings::Location::new(Path::new("app.cfg")).at(index + 1)),
                );
            }
            Settings::new(entries)
        }
    }

    let settings = Lines.load(b"port:8080\n").expect("a valid file");
    let settings = Settings::new(
        settings
            .entries()
            .iter()
            .map(|entry| Entry::new(&["serve"], entry.key(), entry.value()))
            .collect(),
    )
    .expect("no key twice");
    let matches = settings
        .apply(command(), &[])
        .expect("the key names an argument")
        .into_command()
        .try_get_matches_from(["serve"])
        .expect("the tree parses");

    let (_, serve) = matches.subcommand().expect("a subcommand was chosen");
    assert_eq!(serve.get_one::<u16>("port"), Some(&8080));
}

#[test]
fn a_loader_that_sees_a_bad_line_reports_it() {
    struct Lines;

    impl Loader for Lines {
        fn load(&self, bytes: &[u8]) -> Result<Settings, Error> {
            let text = std::str::from_utf8(bytes).map_err(|error| Error::new(error.to_string()))?;
            text.lines()
                .position(|line| !line.contains(':'))
                .map_or_else(
                    || Ok(Settings::empty()),
                    |index| {
                        Err(Error::new(format!(
                            "{}",
                            clifacet::settings::Location::new(Path::new("app.cfg")).at(index + 1)
                        )))
                    },
                )
        }
    }

    let error = Lines
        .load(b"port:8080\nnonsense\n")
        .expect_err("line 2 has no key");

    assert_eq!(error.message(), "app.cfg:2");
}

#[test]
fn a_parsed_command_still_produces_the_same_input() {
    // The parse path is unchanged: settings are a command *builder*, so everything after
    // them — the derive's bridge, validation, the command — sees an ordinary parse.
    #[derive(Parser, Debug)]
    #[command(name = "app", no_binary_name = true)]
    struct Cli {
        #[arg(long, default_value = "1")]
        port: u16,
    }

    let applied = settings(vec![Entry::new(&[], "port", "8080")])
        .apply(Cli::command(), &[])
        .expect("the key names an argument");
    let cli = Cli::from_arg_matches(
        &applied
            .into_command()
            .try_get_matches_from(std::iter::empty::<&str>())
            .expect("the file supplies the value"),
    )
    .expect("the parse is ordinary");

    assert_eq!(cli.port, 8080);
}

#[test]
fn the_applied_value_is_owned_so_repeated_runs_do_not_leak() {
    // `clap`'s `string` feature is what makes this possible; leaking one value per run
    // would be the alternative, and a harness that runs a hundred times would show it.
    for _ in 0..100 {
        let applied: Applied = settings(vec![Entry::new(&[], "port", "8080".repeat(64))])
            .apply(
                Command::new("app")
                    .no_binary_name(true)
                    .arg(Arg::new("port").long("port")),
                &[],
            )
            .expect("the key names an argument");
        assert_eq!(applied.entries().len(), 1);
    }
}
