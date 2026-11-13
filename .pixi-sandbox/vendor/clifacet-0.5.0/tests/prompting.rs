//! The prompt adapter's promise: ask only when allowed, and only for what is missing.
//!
//! decision-3 keeps the mode decision and the frontend apart. This file tests the
//! frontend side of that line — the session that decides whether a question may be
//! asked at all, the scripted adapter the framework owns for tests, and the
//! terminal adapter driven here through in-memory streams instead of a TTY.

use std::cell::Cell;
use std::io::Cursor;

use clifacet::policy::{Capabilities, Mode, Plan, Policy, Request};
use clifacet::prompt::{Answer, Choice, Confirmation, Prompter, Scripted, Session, Terminal};
use clifacet::provenance::Source;
use clifacet::secret::Secret;
use clifacet::{Command, Failure, FailureKind};
use rstest::rstest;

/// A plan that may ask the user anything.
fn interactive() -> Plan {
    Policy::up_to(Mode::Interactive)
        .resolve(Request::none(), Capabilities::terminal())
        .expect("prompts are permitted here")
}

/// A plan that may ask nothing at all.
fn non_interactive() -> Plan {
    Policy::non_interactive()
        .resolve(Request::none(), Capabilities::headless())
        .expect("the default resolves")
}

/// A plan that may ask nothing, but whose confirmations answer themselves.
fn non_interactive_assuming_yes() -> Plan {
    Policy::non_interactive()
        .resolve(Request::none().assuming_yes(true), Capabilities::headless())
        .expect("the default resolves")
}

/// A plan whose confirmations answer themselves.
fn interactive_assuming_yes() -> Plan {
    Policy::up_to(Mode::Interactive)
        .resolve(Request::none().assuming_yes(true), Capabilities::terminal())
        .expect("prompts are permitted here")
}

#[rstest]
#[case::prompted(interactive(), None, Some("ada"), 1)]
#[case::supplied_is_not_re_asked(interactive(), Some("ada"), Some("ada"), 0)]
#[case::nothing_can_ask(non_interactive(), None, None, 0)]
#[case::supplied_without_a_frontend(non_interactive(), Some("ada"), Some("ada"), 0)]
fn a_missing_value_is_prompted_only_when_permitted(
    #[case] plan: Plan,
    #[case] supplied: Option<&str>,
    #[case] expected: Option<&str>,
    #[case] expected_asks: usize,
) {
    let script = Scripted::new([Answer::Text(String::from("ada"))]);
    let probe = script.clone();
    let mut session = Session::start(plan, move || Box::new(probe));

    let collected = session
        .value(supplied.map(str::to_owned), "Who should I greet?")
        .expect("this answer is usable");

    assert_eq!(collected.as_deref(), expected);
    assert_eq!(
        script.asked().len(),
        expected_asks,
        "asked: {:?}",
        script.asked()
    );
}

#[rstest]
#[case::answers_yes(interactive(), Answer::Yes, Confirmation::Confirmed, 1)]
#[case::answers_no(interactive(), Answer::No, Confirmation::Declined, 1)]
#[case::yes_flag_skips_the_question(
    interactive_assuming_yes(),
    Answer::No,
    Confirmation::Assumed,
    0
)]
#[case::yes_flag_without_a_frontend(
    non_interactive_assuming_yes(),
    Answer::No,
    Confirmation::Assumed,
    0
)]
#[case::nothing_can_ask(non_interactive(), Answer::Yes, Confirmation::Unavailable, 0)]
fn a_confirmation_says_how_it_was_settled(
    #[case] plan: Plan,
    #[case] answer: Answer,
    #[case] expected: Confirmation,
    #[case] expected_asks: usize,
) {
    let script = Scripted::new([answer]);
    let probe = script.clone();
    let mut session = Session::start(plan, move || Box::new(probe));

    let settled = session
        .confirm("Fail on purpose?")
        .expect("this answer works");

    assert_eq!(settled, expected);
    assert_eq!(
        script.asked().len(),
        expected_asks,
        "asked: {:?}",
        script.asked()
    );
}

/// The boolean a caller that does not care *how* still gets is the old fail-safe one:
/// only a yes, or `--yes`, proceeds. An unanswerable confirmation is not consent.
#[rstest]
#[case::confirmed(Confirmation::Confirmed, true)]
#[case::assumed(Confirmation::Assumed, true)]
#[case::declined(Confirmation::Declined, false)]
#[case::unavailable(Confirmation::Unavailable, false)]
fn only_a_yes_or_the_yes_flag_counts_as_confirmed(
    #[case] settled: Confirmation,
    #[case] proceeds: bool,
) {
    assert_eq!(settled.is_confirmed(), proceeds);
}

/// "No" and "I stopped" are different answers: one is a decision the flow may act
/// on, the other ends the flow. Only the second is a cancellation.
#[test]
fn a_dismissed_confirmation_is_a_cancellation_not_a_no() {
    let declined = Scripted::new([Answer::No]);
    let mut session = Session::start(interactive(), {
        let probe = declined.clone();
        move || Box::new(probe)
    });
    assert_eq!(
        session
            .confirm("Fail on purpose?")
            .expect("no is an answer"),
        Confirmation::Declined,
        "a refusal to proceed is an answer the caller acts on"
    );

    let dismissed = Scripted::new([Answer::Cancel]);
    let mut session = Session::start(interactive(), {
        let probe = dismissed.clone();
        move || Box::new(probe)
    });
    let failure = session
        .confirm("Fail on purpose?")
        .expect_err("dismissing ends the flow");

    assert_eq!(failure.kind(), FailureKind::Cancelled);
    assert_ne!(failure.kind(), FailureKind::Validation);
}

#[test]
fn a_cancelled_question_reports_cancellation() {
    let script = Scripted::new([Answer::Cancel]);
    let probe = script.clone();
    let mut session = Session::start(interactive(), move || Box::new(probe));

    let failure = session
        .value(None, "Who should I greet?")
        .expect_err("a cancel produces no value");

    assert_eq!(failure.kind(), FailureKind::Cancelled);
    assert!(
        !failure.message().is_empty(),
        "a cancel says what was stopped"
    );
}

#[test]
fn a_script_answering_the_wrong_kind_of_question_fails_loudly() {
    let script = Scripted::new([Answer::Yes]);
    let probe = script.clone();
    let mut session = Session::start(interactive(), move || Box::new(probe));

    let failure = session
        .value(None, "Who should I greet?")
        .expect_err("a confirmation is not a name");

    assert_eq!(failure.kind(), FailureKind::Execution);
    assert!(
        failure.message().contains("not text"),
        "the message explains the mismatch: {}",
        failure.message()
    );
}

#[test]
fn a_script_that_runs_out_fails_instead_of_guessing() {
    let script = Scripted::default();
    let probe = script.clone();
    let mut session = Session::start(interactive(), move || Box::new(probe));

    let failure = session
        .value(None, "Who should I greet?")
        .expect_err("an exhausted script cannot answer");

    assert_eq!(failure.kind(), FailureKind::Execution);
    assert!(
        failure.message().contains("script"),
        "the message names the cause: {}",
        failure.message()
    );
}

#[rstest]
#[case::yes("y\n", true)]
#[case::yes_word("yes\n", true)]
#[case::no("n\n", false)]
#[case::no_word("NO\n", false)]
#[case::repeats_until_clear("maybe\nyes\n", true)]
fn a_terminal_confirmation_reads_the_answer(#[case] typed: &str, #[case] expected: bool) {
    let mut prompt = Vec::new();
    let confirmed = {
        let mut terminal = Terminal::new(Cursor::new(typed), &mut prompt);
        clifacet::prompt::Prompter::confirm(&mut terminal, "Fail on purpose?")
    };

    assert_eq!(
        confirmed.expect("the input ends with a clear answer"),
        expected
    );
    let shown = String::from_utf8(prompt).expect("the prompt stream is UTF-8");
    assert!(shown.contains("Fail on purpose?"), "shown: {shown}");
}

#[test]
fn a_terminal_question_keeps_the_answer_and_trims_it() {
    let mut prompt = Vec::new();
    let answer = {
        let mut terminal = Terminal::new(Cursor::new("  ada  \n"), &mut prompt);
        clifacet::prompt::Prompter::ask(&mut terminal, "Who should I greet?")
    };

    assert_eq!(answer.expect("a line was given"), "ada");
}

#[test]
fn end_of_input_is_a_cancellation_not_an_empty_answer() {
    let mut prompt = Vec::new();
    let answer = {
        let mut terminal = Terminal::new(Cursor::new(""), &mut prompt);
        clifacet::prompt::Prompter::ask(&mut terminal, "Who should I greet?")
    };

    assert_eq!(
        answer.expect_err("closing the input cancels").kind(),
        FailureKind::Cancelled
    );
}

/// Counts executions, so "the handler did not run" is observable.
#[derive(Default)]
struct Counting {
    runs: Cell<usize>,
}

impl Command for Counting {
    type Input = ();
    type Output = usize;

    fn execute(&self, (): ()) -> Result<Self::Output, Failure> {
        self.runs.set(self.runs.get() + 1);
        Ok(self.runs.get())
    }
}

#[test]
fn a_collection_failure_never_reaches_the_handler() {
    let command = Counting::default();
    let script = Scripted::default();
    let probe = script.clone();
    let mut session = Session::start(interactive(), move || Box::new(probe));

    let outcome = session.run(&command, |_session| {
        Err(Failure::validation("the answer was no good"))
    });

    assert_eq!(
        outcome.expect_err("collection failed").kind(),
        FailureKind::Validation
    );
    assert_eq!(command.runs.get(), 0, "nothing executed");
}

#[test]
fn a_collected_input_runs_exactly_once() {
    let command = Counting::default();
    let script = Scripted::default();
    let probe = script.clone();
    let mut session = Session::start(interactive(), move || Box::new(probe));

    let runs = session
        .run(&command, |_session| Ok(()))
        .expect("collection succeeded");

    assert_eq!(runs, 1);
    assert_eq!(command.runs.get(), 1);
}

#[test]
fn a_session_reports_the_plan_it_was_given() {
    let script = Scripted::default();
    let probe = script.clone();

    let session = Session::start(interactive(), move || Box::new(probe));

    assert_eq!(session.plan(), interactive());
    assert!(session.plan().asks_the_user());
}

#[test]
fn a_script_reports_how_many_answers_it_has_left() {
    let script = Scripted::new([Answer::Text(String::from("ada")), Answer::Cancel]);
    assert_eq!(script.remaining(), 2);

    let probe = script.clone();
    let mut session = Session::start(interactive(), move || Box::new(probe));
    session
        .value(None, "Who should I greet?")
        .expect("the answer exists");

    assert_eq!(script.remaining(), 1, "an answer is consumed, not copied");
    assert_eq!(script.asked(), vec![String::from("Who should I greet?")]);
}

#[test]
fn a_text_answer_to_a_confirmation_fails_loudly() {
    let script = Scripted::new([Answer::Text(String::from("maybe"))]);
    let probe = script.clone();
    let mut session = Session::start(interactive(), move || Box::new(probe));

    let failure = session
        .confirm("Fail on purpose?")
        .expect_err("text is not a confirmation");

    assert_eq!(failure.kind(), FailureKind::Execution);
    assert!(
        failure.message().contains("yes or no"),
        "the message explains the mismatch: {}",
        failure.message()
    );
}

#[test]
fn a_session_explains_itself_when_debugged() {
    let script = Scripted::default();
    let probe = script.clone();
    let session = Session::start(interactive(), move || Box::new(probe));

    let described = format!("{session:?}");
    assert!(described.contains("Session"), "{described}");
    assert!(described.contains("constructed"), "{described}");
}

/// The statuses a choice question offers in these tests.
const STATUSES: [&str; 3] = ["to do", "in progress", "done"];

#[rstest]
#[case::prompted(interactive(), None, Some("in progress"), 1)]
#[case::supplied_is_not_re_asked(interactive(), Some("done"), Some("done"), 0)]
#[case::supplied_is_untouched_even_if_unoffered(interactive(), Some("blocked"), Some("blocked"), 0)]
#[case::nothing_can_ask(non_interactive(), None, None, 0)]
fn a_choice_is_asked_only_when_missing(
    #[case] plan: Plan,
    #[case] supplied: Option<&'static str>,
    #[case] expected: Option<&str>,
    #[case] expected_asks: usize,
) {
    let script = Scripted::new([Answer::Text(String::from("in progress"))]);
    let probe = script.clone();
    let mut session = Session::start(plan, move || Box::new(probe));

    let chosen = session
        .choose(supplied, "Which status?", STATUSES)
        .expect("this answer is one of the choices");

    assert_eq!(chosen, expected);
    assert_eq!(
        script.asked().len(),
        expected_asks,
        "asked: {:?}",
        script.asked()
    );
}

/// The value handed back is the option itself, not its label read back and parsed.
#[test]
fn a_choice_hands_back_the_offered_value() {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Level {
        Quiet,
        Loud,
    }
    impl std::fmt::Display for Level {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(match self {
                Self::Quiet => "quiet",
                Self::Loud => "loud",
            })
        }
    }
    let mut session = Session::start(interactive(), || {
        Box::new(Scripted::new([Answer::Text(String::from("loud"))]))
    });

    let chosen = session
        .choose(None, "How loud?", [Level::Quiet, Level::Loud])
        .expect("a label that was offered");

    assert_eq!(chosen, Some(Level::Loud));
}

#[rstest]
#[case::an_unoffered_label(Answer::Text(String::from("blocked")), FailureKind::Execution, "done")]
#[case::a_yes(Answer::Yes, FailureKind::Execution, "Which status?")]
#[case::a_dismissal(Answer::Cancel, FailureKind::Cancelled, "Which status?")]
fn a_script_that_cannot_choose_fails_loudly(
    #[case] answer: Answer,
    #[case] kind: FailureKind,
    #[case] mentioned: &str,
) {
    let mut session = Session::start(interactive(), move || Box::new(Scripted::new([answer])));

    let failure = session
        .choose(None, "Which status?", STATUSES)
        .expect_err("nothing usable was chosen");

    assert_eq!(failure.kind(), kind, "{}", failure.message());
    assert!(
        failure.message().contains(mentioned),
        "{}",
        failure.message()
    );
}

/// A choice with nothing to choose from is a declaration mistake, found before asking.
#[test]
fn a_choice_without_options_is_refused_before_anything_is_asked() {
    let script = Scripted::default();
    let probe = script.clone();
    let mut session = Session::start(interactive(), move || Box::new(probe));

    let failure = session
        .choose(None, "Which status?", [""; 0])
        .expect_err("there is nothing to choose");

    assert_eq!(failure.kind(), FailureKind::Execution);
    assert_eq!(script.asked(), [""; 0].map(String::from));
}

/// The statuses as a frontend sees them: labels only.
fn status_choices() -> Vec<Choice> {
    STATUSES.into_iter().map(Choice::new).collect()
}

#[rstest]
#[case::by_number("2\n", 1)]
#[case::by_name_in_any_case("Done\n", 2)]
#[case::after_a_typo("dun\n3\n", 2)]
#[case::after_numbers_out_of_range("0\n4\n1\n", 0)]
fn a_terminal_choice_takes_a_number_or_a_name(#[case] typed: &str, #[case] expected: usize) {
    let mut prompt = Vec::new();
    let chosen = {
        let mut terminal = Terminal::new(Cursor::new(typed), &mut prompt);
        terminal.choose("Which status?", &status_choices())
    };

    assert_eq!(chosen.expect("an answer that names a choice"), expected);
    let shown = String::from_utf8(prompt).expect("the prompt stream is UTF-8");
    assert!(
        shown.contains("Which status?") && shown.contains("2. in progress"),
        "the choices are listed, numbered: {shown:?}"
    );
}

#[test]
fn a_terminal_choice_shows_the_help_beside_the_label() {
    let mut prompt = Vec::new();
    {
        let mut terminal = Terminal::new(Cursor::new("1\n"), &mut prompt);
        let choices = [Choice::new("done").with_help("finished and verified")];
        terminal
            .choose("Which status?", &choices)
            .expect("one was named");
    }

    let shown = String::from_utf8(prompt).expect("the prompt stream is UTF-8");
    assert!(
        shown.contains("1. done — finished and verified"),
        "{shown:?}"
    );
}

#[test]
fn a_terminal_choice_cancels_when_the_input_ends() {
    let mut terminal = Terminal::new(Cursor::new(""), std::io::sink());

    let failure = terminal
        .choose("Which status?", &status_choices())
        .expect_err("nobody is there to choose");

    assert_eq!(failure.kind(), FailureKind::Cancelled);
}

/// A frontend written before choices existed: it can only ask for text.
struct TextOnly(Vec<String>);

impl Prompter for TextOnly {
    fn ask(&mut self, question: &str) -> Result<String, Failure> {
        assert!(
            question.contains("1. to do"),
            "the choices are listed: {question}"
        );
        Ok(self.0.remove(0))
    }

    fn confirm(&mut self, _question: &str) -> Result<bool, Failure> {
        unreachable!("no confirmation is asked here")
    }
}

/// `choose` is a provided method, so a frontend that only knows `ask` still offers a
/// choice — as a numbered list it reads one answer for — and rejects what is not one.
#[rstest]
#[case::by_number("3", Ok("done"))]
#[case::by_name("In Progress", Ok("in progress"))]
#[case::neither("someday", Err(FailureKind::Validation))]
fn a_frontend_that_only_asks_text_still_offers_a_choice(
    #[case] typed: &str,
    #[case] expected: Result<&str, FailureKind>,
) {
    let frontend = TextOnly(vec![typed.to_owned()]);
    let mut session = Session::start(interactive(), move || Box::new(frontend));

    let chosen = session.choose(None, "Which status?", STATUSES);

    assert_eq!(
        chosen
            .map(|value| value.expect("asked"))
            .map_err(|failure| failure.kind()),
        expected
    );
}

/// A frontend that answers with a position it was never offered.
struct OutOfRange;

impl Prompter for OutOfRange {
    fn ask(&mut self, _question: &str) -> Result<String, Failure> {
        unreachable!("only a choice is asked here")
    }

    fn confirm(&mut self, _question: &str) -> Result<bool, Failure> {
        unreachable!("only a choice is asked here")
    }

    fn choose(&mut self, _question: &str, choices: &[Choice]) -> Result<usize, Failure> {
        Ok(choices.len())
    }
}

#[test]
fn a_frontend_choosing_outside_the_list_is_a_frontend_failure() {
    let mut session = Session::start(interactive(), || Box::new(OutOfRange));

    let failure = session
        .choose(None, "Which status?", STATUSES)
        .expect_err("position 3 of 3 does not exist");

    assert_eq!(failure.kind(), FailureKind::Execution);
}

#[test]
fn a_choice_carries_a_label_and_optional_help() {
    let plain = Choice::new("done");
    let explained = Choice::new("done").with_help("finished and verified");

    assert_eq!((plain.label(), plain.help()), ("done", None));
    assert_eq!(explained.help(), Some("finished and verified"));
}

// --- Secrets: asked without echo, recorded as secret, reachable only through `Secret` ---

const TOKEN: &str = "hunter2-correct-horse";

#[rstest]
#[case::prompted(interactive(), None, Some(TOKEN), 1)]
#[case::supplied_is_not_re_asked(interactive(), Some("from-the-flag"), Some("from-the-flag"), 0)]
#[case::nothing_can_ask(non_interactive(), None, None, 0)]
fn a_secret_is_asked_only_when_missing(
    #[case] plan: Plan,
    #[case] supplied: Option<&str>,
    #[case] expected: Option<&str>,
    #[case] expected_asks: usize,
) {
    let script = Scripted::new([Answer::Text(TOKEN.to_owned())]);
    let probe = script.clone();
    let mut session = Session::start(plan, move || Box::new(probe));

    let secret = session
        .ask_secret(
            supplied.map(|value| Secret::new(value.to_owned())),
            "token",
            "API token?",
        )
        .expect("this answer is usable");

    assert_eq!(
        secret.as_ref().map(|secret| secret.expose().as_str()),
        expected
    );
    assert_eq!(
        script.asked().len(),
        expected_asks,
        "asked: {:?}",
        script.asked()
    );
}

#[test]
fn a_prompted_secret_is_recorded_as_a_secret_from_the_prompt() {
    let mut session = Session::start(interactive(), || {
        Box::new(Scripted::new([Answer::Text(TOKEN.to_owned())]))
    });

    session
        .ask_secret(None, "token", "API token?")
        .expect("answered");

    assert_eq!(
        session.provenance().source_of("token"),
        Some(Source::Prompt)
    );
    assert!(session.provenance().is_secret("token"));
}

/// Where a supplied secret came from — argv or the environment — is known where it was
/// parsed, not here; the session records only what its own questions produced.
#[test]
fn a_supplied_secret_is_not_attributed_to_the_prompt() {
    let mut session = Session::start(interactive(), || Box::new(Scripted::default()));

    session
        .ask_secret(Some(Secret::new(TOKEN.to_owned())), "token", "API token?")
        .expect("never asked");

    assert_eq!(session.provenance().source_of("token"), None);
}

/// The value exists in one place, `Secret`; every other way to look at the session
/// sees its name and where it came from, never the text.
#[test]
fn a_prompted_secret_is_reachable_only_through_secret() {
    let mut session = Session::start(interactive(), || {
        Box::new(Scripted::new([Answer::Text(TOKEN.to_owned())]))
    });

    let secret = session
        .ask_secret(None, "token", "API token?")
        .expect("answered")
        .expect("asked");

    assert_eq!(secret.expose(), TOKEN);
    for shown in [
        format!("{secret:?}"),
        format!("{session:?}"),
        format!("{}", session.provenance()),
        format!("{:?}", session.provenance()),
    ] {
        assert!(!shown.contains(TOKEN), "the secret leaked into {shown:?}");
    }
}

#[test]
fn a_dismissed_secret_is_a_cancellation_and_records_nothing() {
    let mut session = Session::start(interactive(), || Box::new(Scripted::new([Answer::Cancel])));

    let failure = session
        .ask_secret(None, "token", "API token?")
        .expect_err("a cancel produces no value");

    assert_eq!(failure.kind(), FailureKind::Cancelled);
    assert!(session.provenance().is_empty());
}

/// A line-reading terminal cannot switch echo off, and echoing a secret is worse than
/// not asking: it refuses before showing the question or reading a byte.
#[test]
fn a_line_terminal_refuses_to_ask_for_a_secret_rather_than_echo_it() {
    let mut prompt = Vec::new();
    let mut input = Cursor::new(format!("{TOKEN}\n"));
    let failure = {
        let mut terminal = Terminal::new(&mut input, &mut prompt);
        terminal
            .ask_secret("API token?")
            .expect_err("it cannot hide what is typed")
    };

    assert_eq!(failure.kind(), FailureKind::Execution);
    assert!(failure.message().contains("echo"), "{}", failure.message());
    assert_eq!(prompt, b"", "the question was never shown");
    assert_eq!(input.position(), 0, "nothing was read");
}

/// The provided `ask_secret` refuses too, so a frontend written before secrets existed
/// cannot be tricked into echoing one through its `ask`.
#[test]
fn a_frontend_without_a_secret_question_refuses_instead_of_asking_in_the_clear() {
    // `ask` would panic on the empty list: the refusal must come first.
    let mut session = Session::start(interactive(), || Box::new(TextOnly(Vec::new())));

    let failure = session
        .ask_secret(None, "token", "API token?")
        .expect_err("this frontend cannot hide input");

    assert_eq!(failure.kind(), FailureKind::Execution);
    assert!(session.provenance().is_empty());
}

// --- Lists: one question repeated until the user stops; never a partial success ---

fn texts(answers: &[&str]) -> Vec<Answer> {
    answers
        .iter()
        .map(|answer| Answer::Text((*answer).to_owned()))
        .collect()
}

fn owned(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[rstest]
#[case::until_an_empty_answer(interactive(), &[], &["cli", "docs", ""], &["cli", "docs"], 3)]
#[case::a_blank_answer_also_stops(interactive(), &[], &["cli", "   "], &["cli"], 2)]
#[case::stopping_at_once_is_an_empty_list(interactive(), &[], &[""], &[], 1)]
#[case::supplied_is_not_re_asked(interactive(), &["ops"], &["cli", ""], &["ops"], 0)]
#[case::nothing_can_ask(non_interactive(), &[], &["cli", ""], &[], 0)]
fn a_list_is_collected_only_when_missing(
    #[case] plan: Plan,
    #[case] supplied: &[&str],
    #[case] answers: &[&str],
    #[case] expected: &[&str],
    #[case] expected_asks: usize,
) {
    let script = Scripted::new(texts(answers));
    let probe = script.clone();
    let mut session = Session::start(plan, move || Box::new(probe));

    let collected = session
        .collect_values(owned(supplied), "label", "Label (empty to finish)?")
        .expect("the list was finished");

    assert_eq!(collected, owned(expected));
    assert_eq!(
        script.asked().len(),
        expected_asks,
        "asked: {:?}",
        script.asked()
    );
    assert!(
        script
            .asked()
            .iter()
            .all(|asked| asked == "Label (empty to finish)?"),
        "the same question each time: {:?}",
        script.asked()
    );
}

/// Stopping part-way is not finishing: the values gathered so far are discarded, so a
/// caller can never mistake two of three labels for the list the user meant.
#[rstest]
#[case::dismissed_part_way(&[Answer::Text(String::from("cli")), Answer::Cancel], FailureKind::Cancelled)]
#[case::script_runs_out_part_way(&[Answer::Text(String::from("cli"))], FailureKind::Execution)]
#[case::wrong_kind_of_answer_part_way(&[Answer::Text(String::from("cli")), Answer::Yes], FailureKind::Execution)]
fn a_list_stopped_part_way_is_never_a_success(
    #[case] answers: &[Answer],
    #[case] kind: FailureKind,
) {
    let script = Scripted::new(answers.to_vec());
    let mut session = Session::start(interactive(), move || Box::new(script));

    let failure = session
        .collect_values(Vec::new(), "label", "Label?")
        .expect_err("a partial list is not returned");

    assert_eq!(failure.kind(), kind, "{}", failure.message());
    assert!(
        session.provenance().is_empty(),
        "an unfinished list records nothing"
    );
}

#[test]
fn a_finished_list_is_recorded_as_prompted() {
    let mut session = Session::start(interactive(), || {
        Box::new(Scripted::new(texts(&["cli", ""])))
    });

    session
        .collect_values(Vec::new(), "label", "Label?")
        .expect("finished");

    assert_eq!(
        session.provenance().source_of("label"),
        Some(Source::Prompt)
    );
    assert!(!session.provenance().is_secret("label"));
}

#[test]
fn a_supplied_or_unaskable_list_is_not_recorded() {
    let mut supplied = Session::start(interactive(), || Box::new(Scripted::default()));
    supplied
        .collect_values(owned(&["ops"]), "label", "Label?")
        .expect("never asked");

    let mut unaskable = Session::start(non_interactive(), || Box::new(Scripted::default()));
    unaskable
        .collect_values(Vec::new(), "label", "Label?")
        .expect("nothing may ask");

    assert!(supplied.provenance().is_empty());
    assert!(unaskable.provenance().is_empty());
}
