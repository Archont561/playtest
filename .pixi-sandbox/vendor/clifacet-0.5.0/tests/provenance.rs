//! Provenance derived from the parse, so where a value came from is a property of the
//! framework rather than of diligence (decision-18 item 4).
//!
//! The three cases an eager implementation gets wrong are the ones tested hardest: a
//! default is not something the user typed, an environment variable is not a flag, and
//! an absent value has no source at all.

use clap::{CommandFactory, Parser, Subcommand};
use clifacet::policy::{Capabilities, Mode, Plan, Policy, Request};
use clifacet::prompt::{Answer, Scripted, Session};
use clifacet::provenance::{Origin, Provenance, Source};
use rstest::rstest;

/// Every way a value can arrive: typed, defaulted, from the environment, or not at all.
#[derive(Debug, Parser)]
#[command(name = "deploy")]
struct Deploy {
    /// Asked for when missing; absent otherwise.
    #[arg(long)]
    name: Option<String>,

    /// Defaulted.
    #[arg(long, default_value_t = 1)]
    repeat: u8,

    /// Read from the environment when not typed. `PATH` is set on every platform the
    /// tests run on, so no test has to mutate the process environment.
    #[arg(long, env = "PATH")]
    path: Option<String>,

    /// A flag: its absence is clap's default of `false`.
    #[arg(long)]
    dry_run: bool,

    /// A credential.
    #[arg(long)]
    token: Option<String>,
}

fn parsed(argv: &[&str]) -> Provenance {
    let command = Deploy::command();
    let matches = command
        .clone()
        .try_get_matches_from(std::iter::once("deploy").chain(argv.iter().copied()))
        .expect("a valid invocation");
    Provenance::from_matches(&command, &matches)
}

#[rstest]
#[case::typed(&["--name", "ada"], "name", Some(Source::Argv))]
#[case::absent_is_not_recorded(&[], "name", None)]
#[case::defaulted(&[], "repeat", Some(Source::Default))]
#[case::a_typed_default_is_typed(&["--repeat", "3"], "repeat", Some(Source::Argv))]
#[case::from_the_environment(&[], "path", Some(Source::Environment))]
#[case::typed_beats_the_environment(&["--path", "/opt"], "path", Some(Source::Argv))]
#[case::an_absent_flag_is_its_default(&[], "dry_run", Some(Source::Default))]
#[case::a_given_flag_is_typed(&["--dry-run"], "dry_run", Some(Source::Argv))]
fn each_present_value_is_recorded_with_where_clap_found_it(
    #[case] argv: &[&str],
    #[case] name: &str,
    #[case] expected: Option<Source>,
) {
    assert_eq!(parsed(argv).source_of(name), expected);
}

/// The derive adds an argument group named after the struct; a group is not a value.
#[test]
fn only_arguments_are_recorded() {
    let provenance = parsed(&["--name", "ada"]);
    let mut names: Vec<&str> = provenance.entries().iter().map(Origin::name).collect();
    names.sort_unstable();

    assert_eq!(names, ["dry_run", "name", "path", "repeat"]);
}

#[derive(Debug, Parser)]
#[command(name = "tool")]
struct Tool {
    /// Accepted at any depth.
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Verb,
}

#[derive(Debug, Subcommand)]
enum Verb {
    /// Run something.
    Run {
        /// What to run.
        #[arg(long)]
        target: String,
    },
}

#[test]
fn the_chosen_subcommand_is_walked_too() {
    let command = Tool::command();
    let matches = command
        .clone()
        .try_get_matches_from(["tool", "run", "--target", "web", "--verbose"])
        .expect("a valid invocation");

    let provenance = Provenance::from_matches(&command, &matches);

    assert_eq!(provenance.source_of("target"), Some(Source::Argv));
    assert_eq!(provenance.source_of("verbose"), Some(Source::Argv));
}

fn interactive() -> Plan {
    Policy::up_to(Mode::Interactive)
        .resolve(Request::none(), Capabilities::terminal())
        .expect("prompts are permitted here")
}

/// A canary no record may ever contain.
const CANARY: &str = "hunter2-canary";

#[test]
fn a_prompted_value_and_a_parsed_value_compose_into_one_record() {
    let mut provenance = parsed(&["--token", CANARY]).with_secrets(&["token"]);
    let mut session = Session::start(interactive(), || {
        Box::new(Scripted::new([Answer::Text(String::from("ada"))]))
    });
    let mut name: Option<String> = None;
    session
        .ask_for(&mut name, &Deploy::command(), "name", "Who?")
        .expect("the answer parses");

    provenance.merge(session.provenance());

    assert_eq!(provenance.source_of("name"), Some(Source::Prompt));
    assert_eq!(provenance.source_of("token"), Some(Source::Argv));
    assert!(provenance.is_secret("token"));
    assert_eq!(provenance.source_of("repeat"), Some(Source::Default));
    let shown = format!("{provenance} {provenance:?}");
    assert!(!shown.contains(CANARY), "a record never holds a value");
    assert!(
        shown.contains("token <- argv (secret)"),
        "the secret is reported as one"
    );
}

#[test]
fn a_later_record_replaces_an_earlier_one_for_the_same_name() {
    let mut provenance = parsed(&[]);
    let mut prompted = Provenance::default();
    prompted.record_secret("repeat", Source::Prompt);

    provenance.merge(&prompted);

    assert_eq!(provenance.source_of("repeat"), Some(Source::Prompt));
    assert!(provenance.is_secret("repeat"));
    assert_eq!(
        provenance
            .entries()
            .iter()
            .filter(|entry| entry.name() == "repeat")
            .count(),
        1
    );
}

#[test]
fn marking_a_secret_that_was_not_supplied_records_nothing() {
    let provenance = parsed(&[]).with_secrets(&["token"]);

    assert_eq!(provenance.source_of("token"), None);
    assert!(!provenance.is_secret("token"));
}
