//! A field declared as a `#[clifacet(..)]` argument parses and reports itself.

#[clifacet::command(input = Greeting)]
#[command(name = "greet", about = "Say hello")]
#[derive(Debug)]
struct Cli {
    /// Who to greet. Defaults to `world`.
    #[clifacet(long, short = 'n', alias = "nickname", value_name = "NAME")]
    name: Option<String>,

    /// How many times to greet.
    #[clifacet(long, value_name = "COUNT")]
    repeat: u8,

    /// Passed to clap untouched, for anything the key table does not cover.
    #[arg(long, value_parser = clap::value_parser!(u8).range(1..=8))]
    level: u8,

    /// `range` is not a clap attribute: it becomes clap's ranged value parser for the
    /// field's own type, `Option` peeled.
    #[clifacet(long, range = 1..=8, visible_alias = "loudness")]
    volume: Option<u8>,

    /// clap's builder keys take their value; `required` is `required = true`.
    #[clifacet(long, required = true, hide = true, alias = "dest")]
    target: String,

    /// `env` and `default_value_t` are clap's bare forms: the field's name in capitals,
    /// and the type's `Default`.
    #[clifacet(long, env, default_value_t)]
    retries: u8,
}

/// The typed input the derive builds field by field.
struct Greeting {
    name: Option<String>,
    repeat: u8,
    level: u8,
    volume: Option<u8>,
    target: String,
    retries: u8,
}

fn main() {
    let cli = <Cli as clifacet::clap::Parser>::try_parse_from([
        "greet",
        "--nickname",
        "ada",
        "--repeat",
        "3",
        "--level",
        "2",
        "--volume",
        "8",
        "--dest",
        "home",
    ])
    .expect("the forwarded grammar parses");
    assert_eq!(cli.name.as_deref(), Some("ada"));
    assert_eq!(cli.repeat, 3);
    assert_eq!(cli.level, 2);
    assert_eq!(cli.volume, Some(8));
    assert_eq!(cli.target, "home", "`alias` is a value key");
    assert_eq!(cli.retries, 0, "bare `default_value_t` is the type's default");

    // The ranged parser is clap's: out of range is a parse error, before any bridge.
    let error = <Cli as clifacet::clap::Parser>::try_parse_from([
        "greet", "--repeat", "1", "--level", "1", "--target", "t", "--volume", "9",
    ])
    .expect_err("9 is outside 1..=8");
    assert!(error.to_string().contains("1..=8"), "clap's own wording: {error}");

    // `required = true` is enforced by clap.
    let missing = <Cli as clifacet::clap::Parser>::try_parse_from(["greet", "--repeat", "1", "--level", "1"])
        .expect_err("--target is required");
    assert!(missing.to_string().contains("--target"), "{missing}");

    // The generated help is clap's, built from the keys the attribute forwarded: the
    // short flag, the value name and the doc comment. `alias` is parsed but, as in clap,
    // not listed — `visible_alias` is the key that shows one.
    let help = <Cli as clap::CommandFactory>::command().render_help().to_string();
    assert!(help.contains("-n, --name <NAME>"), "short and value_name: {help}");
    assert!(help.contains("Who to greet"), "the doc comment is the help: {help}");
    assert!(help.contains("--loudness"), "`visible_alias` is listed: {help}");
    // Hidden from the option list; clap still names a required one in the usage line.
    assert!(
        !help.lines().any(|line| line.trim_start().starts_with("--target")),
        "`hide = true` hides it from the options: {help}"
    );
}
