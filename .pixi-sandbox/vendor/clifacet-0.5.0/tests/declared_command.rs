//! The command as a whole: a rule across fields, a field that is not input, the form.
//!
//! `declared_fields.rs` proves what one field's declaration does. Three things are not
//! about one field, and this file proves them through the same public seams:
//!
//! - `check = path` runs an application rule on the *built* input, after every field
//!   rule — so it sees `fill`ed, trimmed values and its failure is a validation failure;
//! - `skip` leaves a field out of the input while clap still parses it, which is how a
//!   command keeps presentation flags beside the values it runs on;
//! - `form` lists a field in `Facet::form_fields`, in declaration order, so the TUI form
//!   is generated from the declarations instead of a hand-written list.

use clifacet::clap::Parser as _;
use clifacet::{Facet, Failure, FailureKind};
use rstest::rstest;

/// The largest booking the check accepts.
const MAX_SEATS: u8 = 8;

#[clifacet::command(input = Booking, check = check_booking)]
#[command(name = "book")]
struct Cli {
    /// Who the booking is for.
    #[clifacet(long, ask = "Who is it for?", fill = "guest", trim, non_empty, form)]
    name: Option<String>,

    /// How many seats — a `u8`, so 255 parses and the range is the check's to enforce.
    #[clifacet(long, default_value_t = 1, form)]
    seats: u8,

    /// Spelled `--dry-run`: a form field submits under its flag, not its field name.
    #[clifacet(long, form)]
    dry_run: Option<String>,

    /// How to show the result: parsed by clap, not part of the booking.
    #[command(flatten)]
    #[clifacet(skip)]
    display: Display,
}

/// Presentation, kept out of the input.
#[derive(Debug, Clone, Copy, Default, clap::Args)]
struct Display {
    /// Say more.
    #[arg(long)]
    loud: bool,
}

/// The booking: no `display`, because `skip` keeps it out.
#[derive(Debug)]
struct Booking {
    name: String,
    seats: u8,
    dry_run: Option<String>,
}

/// A rule across the built input: it sees the normalised name, not the raw flag.
fn check_booking(booking: &Booking) -> Result<(), Failure> {
    if booking.seats == 0 || booking.seats > MAX_SEATS {
        return Err(Failure::validation(format!(
            "--seats must be between 1 and {MAX_SEATS}, got {}",
            booking.seats
        )));
    }
    if booking.name == "nobody" {
        return Err(Failure::validation("a booking needs a name"));
    }
    Ok(())
}

fn parse(argv: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("book").chain(argv.iter().copied()))
        .expect("the declared grammar parses")
}

#[rstest]
#[case::too_many(&["--seats", "255"], "--seats must be between 1 and 8, got 255")]
#[case::none(&["--seats", "0"], "--seats must be between 1 and 8, got 0")]
// Trimmed before the check sees it: the raw flag is "  nobody  ".
#[case::sees_the_trimmed_value(&["--name", "  nobody  "], "a booking needs a name")]
fn the_check_rejects_the_built_input_as_a_validation_failure(
    #[case] argv: &[&str],
    #[case] message: &str,
) {
    let failure = parse(argv).validate().expect_err("the check rejects this");
    assert_eq!(failure.kind(), FailureKind::Validation);
    assert_eq!(failure.message(), message);
}

#[test]
fn the_check_is_part_of_the_generated_conversion() {
    // `TryFrom` is the bridge, so the check cannot be skipped by converting directly.
    let failure = Booking::try_from(parse(&["--seats", "9"])).expect_err("9 is too many");
    assert_eq!(failure.kind(), FailureKind::Validation);
}

#[test]
fn a_field_rule_fails_before_the_check_runs() {
    // Both are wrong; the field's own rule is the one reported.
    let failure = parse(&["--name", "   ", "--seats", "9"])
        .validate()
        .expect_err("blank and too many");
    assert_eq!(failure.message(), "--name must not be blank");
}

#[test]
fn an_input_the_check_accepts_is_built_from_the_normalised_fields() {
    let booking = parse(&["--seats", "2", "--loud"])
        .validate()
        .expect("two seats for the filled-in guest");
    assert_eq!(booking.name, "guest");
    assert_eq!(booking.seats, 2);
    assert_eq!(booking.dry_run, None);
}

#[test]
fn a_skipped_field_is_still_parsed_by_clap() {
    let cli = parse(&["--loud"]);
    assert!(cli.display.loud, "`skip` leaves the grammar alone");
    cli.validate().expect("and the input is built without it");
}

#[test]
fn the_form_lists_the_declared_fields_in_declaration_order() {
    assert_eq!(<Cli as Facet>::form_fields(), ["name", "seats", "dry_run"]);
}

#[test]
fn a_command_that_declares_no_form_field_has_an_empty_form() {
    #[clifacet::command(input = Plain)]
    struct PlainCli {
        #[clifacet(long)]
        value: Option<String>,
    }
    struct Plain {
        value: Option<String>,
    }

    assert_eq!(<PlainCli as Facet>::form_fields(), [""; 0]);
    let plain = PlainCli::try_parse_from(["plain", "--value", "v"])
        .expect("parses")
        .validate()
        .expect("copied by name");
    assert_eq!(plain.value.as_deref(), Some("v"));
}

#[cfg(feature = "tui")]
#[test]
fn a_generated_form_submits_each_field_under_its_flag() {
    use clifacet::tui::{FormState, Key};

    let mut form = FormState::generated(&<Cli as Facet>::command(), Cli::form_fields())
        .expect("the declared fields are the command's");
    // To the third field, `dry_run`, and type into it.
    for key in [
        Key::Tab,
        Key::Tab,
        Key::Char('n'),
        Key::Char('o'),
        Key::Char('w'),
    ] {
        form.handle(key);
    }

    let arguments = form.arguments();
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == ["--dry-run", "now"]),
        "the flag clap knows, not the field name: {arguments:?}"
    );
    parse(&arguments.iter().map(String::as_str).collect::<Vec<_>>())
        .validate()
        .expect("what the form submits, clap parses");
}

/// A command whose credential is both a form field and secret.
#[clifacet::command(input = Login)]
struct LoginCli {
    /// Who signs in
    #[clifacet(long, form)]
    user: Option<String>,

    /// The API token
    #[clifacet(long, secret, form)]
    token: Option<clifacet::secret::Secret<String>>,
}

struct Login {
    user: Option<String>,
    token: Option<clifacet::secret::Secret<String>>,
}

#[test]
fn the_secret_fields_are_listed_in_declaration_order() {
    assert_eq!(<LoginCli as Facet>::secret_fields(), ["token"]);
    assert_eq!(
        <Cli as Facet>::secret_fields(),
        [""; 0],
        "nothing declared secret"
    );
}

/// A credential that can also come from the environment. `PATH` stands in for the
/// variable because every platform the tests run on sets it, so no test has to mutate
/// the process environment to have a value clap could print.
#[clifacet::command(input = Deploy)]
#[command(name = "deploy")]
struct DeployCli {
    /// The API token
    #[clifacet(long, env = "PATH", secret)]
    token: Option<clifacet::secret::Secret<String>>,
}

struct Deploy {
    token: Option<clifacet::secret::Secret<String>>,
}

/// clap's help prints an environment variable's current value beside its name, so a
/// secret read from one would be shown by `--help` (decision-12): the help names the
/// variable and never its value.
#[test]
fn help_names_a_secret_environment_variable_but_never_shows_its_value() {
    let value = std::env::var("PATH").expect("PATH is set wherever the tests run");
    let help = <DeployCli as Facet>::command().render_help().to_string();

    assert!(
        help.contains("[env: PATH]"),
        "the variable is still named: {help}"
    );
    assert!(
        !help.contains(&value),
        "the value leaked into --help: {help}"
    );

    // Hiding is presentation only: the credential still arrives, and a flag still wins.
    let deploy = DeployCli::try_parse_from(["deploy", "--token", "s3cret"])
        .expect("the flag parses")
        .validate()
        .expect("nothing to reject");
    assert_eq!(
        deploy.token.as_ref().map(|token| token.expose().as_str()),
        Some("s3cret")
    );
}

/// A form built from the declarations masks a `secret` field by itself: forgetting a
/// `mark_secret` call must not be how a token ends up on the screen.
#[cfg(feature = "tui")]
#[test]
fn a_form_built_from_the_declarations_masks_its_secret_fields() {
    use clifacet::tui::{FormState, Key};

    let mut form = FormState::of::<LoginCli>().expect("the declared fields are the command's");
    form.handle(Key::Tab);
    for character in "s3cret".chars() {
        form.handle(Key::Char(character));
    }

    let token = form.field("token").expect("the token is a form field");
    assert!(token.is_secret(), "declared secret, so drawn as the marker");
    assert!(!form.field("user").expect("a form field").is_secret());
    assert_eq!(
        form.arguments(),
        ["--token", "s3cret"],
        "masking is presentation only"
    );
    let login = LoginCli::try_parse_from(
        std::iter::once("login").chain(form.arguments().iter().map(String::as_str)),
    )
    .expect("what the form submits, clap parses")
    .validate()
    .expect("valid");
    assert_eq!(login.token.expect("submitted").expose(), "s3cret");
    assert_eq!(login.user, None);
}

#[cfg(feature = "tui")]
#[test]
fn a_form_built_from_the_declarations_is_the_generated_form_otherwise() {
    use clifacet::tui::FormState;

    assert_eq!(
        FormState::of::<Cli>().expect("declared"),
        FormState::generated(&<Cli as Facet>::command(), Cli::form_fields()).expect("declared")
    );
}
