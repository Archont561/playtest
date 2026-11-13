//! The TOML loader: a table is a section, a key is a long flag, and a bad value says where.
//!
//! `toml` does not keep line numbers in its values, so these tests pin what the loader does
//! instead of a line it cannot know: the message names the file and the dotted key, and
//! the argument's own parser supplies the rest.

#![cfg(feature = "settings-toml")]

use std::path::Path;

use clap::{Arg, Command};
use clifacet::settings::toml::Toml;
use clifacet::settings::{Entry, Settings};

fn tree() -> Command {
    Command::new("app").no_binary_name(true).subcommand(
        Command::new("serve")
            .arg(
                Arg::new("port")
                    .long("port")
                    .value_parser(clap::value_parser!(u16)),
            )
            .arg(Arg::new("host").long("host"))
            .arg(
                Arg::new("quiet")
                    .long("quiet")
                    .action(clap::ArgAction::SetTrue),
            )
            .subcommand(
                Command::new("retry").arg(
                    Arg::new("attempts")
                        .long("attempts")
                        .value_parser(clap::value_parser!(u32)),
                ),
            ),
    )
}

#[test]
fn a_table_is_a_section_and_a_key_is_a_long_flag() {
    let settings = Toml
        .load_from(
            Path::new("app.toml"),
            br#"
            [serve]
            port = 8080
            host = "0.0.0.0"
            "#,
        )
        .expect("a valid file");

    let keys: Vec<(&[String], &str)> = settings
        .entries()
        .iter()
        .map(|entry| (entry.section(), entry.key()))
        .collect();
    assert_eq!(
        keys,
        [
            (&[String::from("serve")][..], "port"),
            (&[String::from("serve")][..], "host")
        ]
    );
}

#[test]
fn a_nested_table_is_a_nested_section() {
    let settings = Toml
        .load_from(
            Path::new("app.toml"),
            b"
            [serve.retry]
            attempts = 3
            ",
        )
        .expect("a valid file");

    let entry = &settings.entries()[0];
    assert_eq!(entry.section(), ["serve".to_owned(), "retry".to_owned()]);
    assert_eq!(entry.key(), "attempts");
    assert_eq!(entry.value(), "3");
}

#[test]
fn a_dotted_header_reaches_the_same_place_as_nesting() {
    // TOML says the two spellings are the same document, so the loader does not need an
    // opinion. The line differs, and should: they are written on different lines.
    let nested = Toml
        .load_from(Path::new("a.toml"), b"[serve]\nport = 1\n")
        .expect("valid");
    let dotted = Toml
        .load_from(Path::new("a.toml"), b"serve.port = 1\n")
        .expect("valid");

    assert_eq!(nested.entries().len(), dotted.entries().len());
    assert_eq!(nested.entries()[0].section(), dotted.entries()[0].section());
    assert_eq!(nested.get(&["serve"], "port").map(Entry::value), Some("1"));
}

#[test]
fn a_non_string_scalar_reaches_the_parser_as_text() {
    // The argument's own parser reads it, so `quiet = true` is a bool flag and `port = 8080`
    // is an integer, without the loader knowing which is which.
    let settings = Toml
        .load_from(
            Path::new("app.toml"),
            b"[serve]\nquiet = true\nport = 8080\n",
        )
        .expect("a valid file");

    assert_eq!(
        settings
            .get(&["serve"], "quiet")
            .map(Entry::value)
            .expect("the key is there"),
        "true"
    );
    let matches = settings
        .apply(tree(), &[])
        .expect("the keys name arguments")
        .into_command()
        .try_get_matches_from(["serve"])
        .expect("the tree parses");
    let (_, serve) = matches.subcommand().expect("a subcommand");
    assert_eq!(serve.get_one::<u16>("port"), Some(&8080));
    assert!(serve.get_flag("quiet"));
}

#[test]
fn an_array_is_refused_because_a_setting_is_one_value() {
    let error = Toml
        .load_from(Path::new("app.toml"), b"[serve]\nport = [1, 2]\n")
        .expect_err("an array is not one value");

    assert!(
        error.message().contains("serve.port"),
        "{}",
        error.message()
    );
    assert!(
        error.message().contains("holds several values"),
        "{}",
        error.message()
    );
}

#[test]
fn a_syntax_error_names_the_file() {
    let error = Toml
        .load_from(Path::new("app.toml"), b"[serve\nport = 1\n")
        .expect_err("the header is unclosed");

    assert!(error.message().contains("app.toml"), "{}", error.message());
}

#[test]
fn an_invalid_value_that_the_loader_cannot_see_is_caught_by_the_parser() {
    // 99999 is a fine TOML integer and not a port. The loader cannot know that; the
    // argument's parser can, and this is where the two halves meet.
    let settings = Toml
        .load_from(Path::new("app.toml"), b"[serve]\nport = 99999\n")
        .expect("valid TOML");

    let error = settings
        .apply(tree(), &[])
        .expect_err("99999 is not a port");

    assert!(error.message().contains("99999"), "{}", error.message());
}

#[test]
fn a_document_that_is_not_a_table_is_refused() {
    // A bare value is not valid TOML at the top level, and the parser says so at the line
    // it found it: the message is the reader's, not a generic refusal.
    let error = Toml
        .load_from(Path::new("app.toml"), b"42\n")
        .expect_err("a document is a table");

    assert!(error.message().contains("app.toml"), "{}", error.message());
    assert!(error.message().contains("line 1"), "{}", error.message());
}

#[test]
fn the_same_key_twice_in_one_section_is_refused() {
    // TOML itself rejects a duplicate key, so this comes from the parser rather than from
    // `Settings::new` — either way the file is refused rather than silently folded.
    let error = Toml
        .load_from(Path::new("app.toml"), b"[serve]\nport = 1\nport = 2\n")
        .expect_err("one key, one value");

    assert!(error.message().contains("app.toml"), "{}", error.message());
}

#[test]
fn a_secret_read_from_toml_is_recorded_without_its_value() {
    let settings = Toml
        .load_from(Path::new("app.toml"), b"[serve]\ntoken = \"hunter2\"\n")
        .expect("valid TOML");
    let applied = settings
        .apply(
            Command::new("app")
                .no_binary_name(true)
                .subcommand(Command::new("serve").arg(Arg::new("token").long("token"))),
            &["token"],
        )
        .expect("the key names an argument");

    assert!(applied.provenance().is_secret("token"));
    assert!(!format!("{}", applied.provenance()).contains("hunter2"));
}

#[test]
fn a_real_file_is_read_from_disk() {
    let directory = std::env::temp_dir().join(format!("clifacet-settings-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("a writable temp directory");
    let path = directory.join("settings.toml");
    std::fs::write(&path, "[serve]\nport = 8080\n").expect("the file writes");

    let settings = Settings::from_toml(&path).expect("a valid file");
    std::fs::remove_dir_all(&directory).ok();

    assert_eq!(
        settings.get(&["serve"], "port").map(Entry::value),
        Some("8080")
    );
}
