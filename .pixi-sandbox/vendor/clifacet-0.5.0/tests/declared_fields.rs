//! One `#[clifacet(..)]` per field: the argument declared once, asked for and normalised.
//!
//! decision-13 makes a field's attribute its whole declaration. The grammar half is
//! clap's and is proved in `tests/ui/field_attribute_valid.rs`; this file proves the
//! other half through the seams an application uses — clap's parser, the generated
//! `collect` on a [`Session`], and the generated `TryFrom` bridge:
//!
//! - `ask` fills a missing value from the user, parsed by the field's own clap parser;
//! - `fill` is the default when neither a flag nor an answer supplied one;
//! - `trim` and `non_empty` keep blank text out of the input;
//! - `confirm` puts a set flag to the user before anything runs;
//! - `secret` keeps a credential redacted, and blank is not a credential.

use clifacet::clap::Parser as _;
use clifacet::policy::{Capabilities, Mode, Plan, Policy, Request};
use clifacet::prompt::{Answer, Collect as _, Scripted, Session};
use clifacet::secret::Secret;
use clifacet::{Facet as _, Failure, FailureKind};
use rstest::rstest;

#[clifacet::command(input = Greeting)]
#[command(name = "greet", about = "Say hello")]
struct Cli {
    /// Who to greet. Defaults to `world`.
    #[clifacet(
        long,
        short = 'n',
        value_name = "NAME",
        ask = "Who should I greet?",
        fill = "world",
        trim,
        non_empty
    )]
    name: Option<String>,

    /// How many times to greet — the parser is an inline expression, and an answer
    /// goes through the very same one.
    #[clifacet(
        long, value_name = "COUNT",
        value_parser = clap::value_parser!(u8).range(1..=8),
        ask = "How many greetings?", fill = 1,
    )]
    count: Option<u8>,

    /// Fail on purpose.
    #[clifacet(long, confirm = "Fail on purpose?")]
    explode: bool,

    /// A credential, never printed; blank is not a credential.
    #[clifacet(long, value_name = "TOKEN", secret, trim, non_empty)]
    token: Option<Secret<String>>,
}

/// Validated input: every value of it is runnable.
#[derive(Debug)]
struct Greeting {
    name: String,
    count: u8,
    explode: bool,
    token: Option<Secret<String>>,
}

fn parse(argv: &[&str]) -> Cli {
    Cli::try_parse_from(std::iter::once("greet").chain(argv.iter().copied()))
        .expect("the declared grammar parses")
}

fn plan(mode: Mode, assume_yes: bool) -> Plan {
    let capabilities = if mode == Mode::NonInteractive {
        Capabilities::headless()
    } else {
        Capabilities::terminal()
    };
    Policy::up_to(mode)
        .resolve(Request::none().assuming_yes(assume_yes), capabilities)
        .expect("this plan resolves")
}

/// Collects through the generated `collect`, then validates through the generated
/// bridge — the order an application runs them in.
fn collected(
    argv: &[&str],
    mode: Mode,
    answers: Vec<Answer>,
) -> (Result<Greeting, Failure>, Vec<String>) {
    let script = Scripted::new(answers);
    let probe = script.clone();
    let mut session = Session::start(plan(mode, false), move || Box::new(probe));
    let result = parse(argv)
        .collect(&mut session)
        .and_then(clifacet::Facet::validate);
    (result, script.asked())
}

#[rstest]
#[case::trimmed(&["--name", "  ada  "], "ada")]
#[case::filled_when_absent(&[], "world")]
fn the_bridge_normalises_what_the_flags_supplied(#[case] argv: &[&str], #[case] expected: &str) {
    let input = parse(argv).validate().expect("this name is valid");
    assert_eq!(input.name, expected);
    assert_eq!(
        input.count, 1,
        "`fill = 1` is the default when nothing supplied a count"
    );
}

#[test]
fn a_blank_required_value_is_a_validation_failure_naming_the_flag() {
    let failure = parse(&["--name", "   "])
        .validate()
        .expect_err("blank is rejected");
    assert_eq!(failure.kind(), FailureKind::Validation);
    assert_eq!(failure.message(), "--name must not be blank");
}

#[test]
fn the_bridge_is_a_generated_try_from() {
    let input =
        Greeting::try_from(parse(&["--name", "grace"])).expect("std's conversion is the bridge");
    assert_eq!(input.name, "grace");
}

#[test]
fn a_missing_value_is_asked_for_and_the_answer_is_normalised() {
    let (input, asked) = collected(
        &[],
        Mode::Interactive,
        vec![
            Answer::Text(String::from("  grace ")),
            Answer::Text(String::from("3")),
        ],
    );
    let input = input.expect("both answers are valid");
    assert_eq!(input.name, "grace");
    assert_eq!(input.count, 3);
    assert_eq!(asked, ["Who should I greet?", "How many greetings?"]);
}

#[test]
fn a_supplied_value_is_never_asked_again() {
    let (input, asked) = collected(
        &["--name", "ada", "--count", "2"],
        Mode::Interactive,
        vec![],
    );
    let input = input.expect("nothing needed asking");
    assert_eq!((input.name.as_str(), input.count), ("ada", 2));
    assert!(asked.is_empty(), "asked: {asked:?}");
}

#[test]
fn nothing_is_asked_when_the_plan_asks_nothing_and_fill_takes_over() {
    let (input, asked) = collected(&[], Mode::NonInteractive, vec![]);
    let input = input.expect("fill supplies both");
    assert_eq!((input.name.as_str(), input.count), ("world", 1));
    assert!(asked.is_empty(), "asked: {asked:?}");
}

#[test]
fn an_answer_is_parsed_by_the_fields_own_clap_parser() {
    let (input, _) = collected(
        &["--name", "ada"],
        Mode::Interactive,
        vec![Answer::Text(String::from("9"))],
    );
    let failure = input.expect_err("9 is outside the declared range");
    assert_eq!(failure.kind(), FailureKind::Validation);
    assert!(
        failure.message().contains("--count"),
        "names the flag: {}",
        failure.message()
    );
    assert!(
        failure.message().contains("1..=8"),
        "clap's own wording: {}",
        failure.message()
    );
}

#[test]
fn a_cancelled_question_cancels_the_collection() {
    let (input, _) = collected(&[], Mode::Interactive, vec![Answer::Cancel]);
    assert_eq!(input.expect_err("cancelled").kind(), FailureKind::Cancelled);
}

#[rstest]
#[case::declined(vec![Answer::No], Some(FailureKind::Cancelled), 1)]
#[case::confirmed(vec![Answer::Yes], None, 1)]
fn a_set_flag_with_confirm_is_put_to_the_user(
    #[case] answers: Vec<Answer>,
    #[case] expected: Option<FailureKind>,
    #[case] expected_asks: usize,
) {
    let (input, asked) = collected(
        &["--name", "ada", "--count", "1", "--explode"],
        Mode::Interactive,
        answers,
    );
    assert_eq!(input.as_ref().err().map(Failure::kind), expected);
    assert_eq!(asked.len(), expected_asks, "asked: {asked:?}");
    if let Err(failure) = input {
        assert!(
            failure.message().contains("--explode"),
            "names the flag: {}",
            failure.message()
        );
    }
}

#[test]
fn a_confirmation_is_not_asked_when_nothing_can_answer_or_yes_already_did() {
    for (mode, assume_yes) in [(Mode::NonInteractive, false), (Mode::Interactive, true)] {
        let script = Scripted::default();
        let probe = script.clone();
        let mut session = Session::start(plan(mode, assume_yes), move || Box::new(probe));
        // Every value is supplied, so the only question left is the confirmation:
        // `--yes` answers confirmations, never a value that is missing.
        let input = parse(&["--name", "ada", "--count", "1", "--explode"])
            .collect(&mut session)
            .and_then(clifacet::Facet::validate)
            .expect("an unasked confirmation does not block a deliberate flag");
        assert!(input.explode);
        assert!(
            script.asked().is_empty(),
            "{mode:?}: asked {:?}",
            script.asked()
        );
    }
}

#[rstest]
#[case::blank_is_absent(&["--token", "   "], None)]
#[case::trimmed(&["--token", " abc "], Some("abc"))]
#[case::absent(&[], None)]
fn a_secret_is_trimmed_and_blank_is_not_a_credential(
    #[case] argv: &[&str],
    #[case] expected: Option<&str>,
) {
    let input = parse(argv)
        .validate()
        .expect("an optional credential is valid");
    assert_eq!(
        input.token.as_ref().map(|token| token.expose().as_str()),
        expected
    );
    assert!(
        !format!("{input:?}").contains("abc"),
        "the input never prints the credential: {input:?}"
    );
}

#[rstest]
#[case::blank("   ")]
#[case::empty("")]
fn a_blank_answer_meets_the_same_rule_as_a_blank_flag(#[case] answer: &str) {
    let (input, _) = collected(
        &["--count", "1"],
        Mode::Interactive,
        vec![Answer::Text(answer.to_owned())],
    );
    assert_eq!(
        input.expect_err("blank is rejected").message(),
        "--name must not be blank"
    );
}

#[test]
fn an_answer_that_starts_with_a_hyphen_is_a_value_not_a_flag() {
    let (input, _) = collected(
        &["--count", "1"],
        Mode::Interactive,
        vec![Answer::Text(String::from("-ada"))],
    );
    assert_eq!(input.expect("a hyphen is text").name, "-ada");
}

/// A command whose other arguments would stop a naive re-parse: one is required, and
/// the asked one is short-only.
#[clifacet::command(input = Strict)]
#[command(name = "strict")]
struct StrictCli {
    /// Required, and not supplied by the answer.
    #[clifacet(long, required = true)]
    target: String,

    /// Short-only, asked for.
    #[clifacet(short = 'l', ask = "Which level?", value_parser = ["low", "high"])]
    level: Option<String>,
}

struct Strict {
    target: String,
    level: Option<String>,
}

#[rstest]
#[case::accepted("high", Ok("high"))]
#[case::rejected("loud", Err("-l"))]
fn an_answer_is_parsed_alone_whatever_else_the_command_requires(
    #[case] answer: &str,
    #[case] expected: Result<&str, &str>,
) {
    let script = Scripted::new([Answer::Text(answer.to_owned())]);
    let mut session = Session::start(plan(Mode::Interactive, false), move || Box::new(script));
    let cli = StrictCli::try_parse_from(["strict", "--target", "t"]).expect("parses");

    let result = cli
        .collect(&mut session)
        .and_then(clifacet::Facet::validate);

    match (result, expected) {
        (Ok(input), Ok(level)) => {
            assert_eq!(input.target, "t");
            assert_eq!(input.level.as_deref(), Some(level));
        }
        (Err(failure), Err(flag)) => {
            assert_eq!(failure.kind(), FailureKind::Validation);
            assert!(
                failure.message().contains(flag),
                "names the flag: {}",
                failure.message()
            );
            assert!(
                failure.message().contains("high"),
                "clap lists the choices: {}",
                failure.message()
            );
        }
        (other, expected) => panic!(
            "expected {expected:?}, got {:?}",
            other.map(|input| input.level)
        ),
    }
}

// --- `Session::ask_for` on its own: a declaration mistake is a failure, never a panic ---

/// A session that may ask, answering every question with `answer`.
fn answering(answer: &str) -> Session {
    let script = Scripted::new([Answer::Text(answer.to_owned())]);
    Session::start(plan(Mode::Interactive, false), move || Box::new(script))
}

fn command() -> clap::Command {
    clap::Command::new("probe")
        .arg(
            clap::Arg::new("count")
                .long("count")
                .value_parser(clap::value_parser!(u8)),
        )
        .arg(clap::Arg::new("target").value_name("TARGET"))
}

#[rstest]
#[case::unknown_argument("missing", "`missing` is not an argument of `probe`")]
#[case::positional_argument("target", "`target` is positional")]
fn asking_for_an_argument_that_cannot_be_asked_for_is_a_declaration_mistake(
    #[case] id: &str,
    #[case] expected: &str,
) {
    let mut slot: Option<String> = None;
    let failure = answering("ada")
        .ask_for(&mut slot, &command(), id, "Which?")
        .expect_err("the declaration is wrong, not the answer");
    assert_eq!(failure.kind(), FailureKind::Execution);
    assert!(
        failure.message().contains(expected),
        "{}",
        failure.message()
    );
    assert_eq!(slot, None, "nothing is filled in");
}

/// An answered question is recorded as coming from the prompt, under its argument's id:
/// the same record a secret or a list of values gets, so where a value came from does
/// not depend on which kind of question produced it.
#[test]
fn an_answered_question_is_recorded_as_prompted() {
    let mut slot: Option<u8> = None;
    let mut session = answering("3");

    session
        .ask_for(&mut slot, &command(), "count", "How many?")
        .expect("3 is a u8");

    assert_eq!(slot, Some(3));
    assert_eq!(
        session.provenance().source_of("count"),
        Some(clifacet::provenance::Source::Prompt)
    );
    assert!(!session.provenance().is_secret("count"));
}

/// A rejected answer fills nothing, so it records nothing either.
#[test]
fn a_rejected_answer_is_not_recorded() {
    let mut slot: Option<u8> = None;
    let mut session = answering("many");

    session
        .ask_for(&mut slot, &command(), "count", "How many?")
        .expect_err("not a u8");

    assert!(session.provenance().is_empty());
}

#[test]
fn an_answer_that_parses_to_another_type_than_the_field_is_a_declaration_mistake() {
    // The argument's parser yields `u8`; the slot says `String`.
    let mut slot: Option<String> = None;
    let failure = answering("3")
        .ask_for(&mut slot, &command(), "count", "How many?")
        .expect_err("the field and its parser disagree");
    assert_eq!(failure.kind(), FailureKind::Execution);
    assert!(
        failure.message().contains("--count"),
        "names the flag: {}",
        failure.message()
    );
    assert!(
        failure.message().contains("different type"),
        "{}",
        failure.message()
    );
}
