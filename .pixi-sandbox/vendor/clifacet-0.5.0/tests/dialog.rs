//! The shipped dialog frontend, on the same seam every other frontend uses.
//!
//! `inquire` needs a real terminal, so what can be asserted here is the half that
//! does not: a run without one fails loudly instead of waiting for an answer that
//! cannot arrive, and a plan that asks nothing never builds the dialog at all.
#![cfg(feature = "interactive")]

use std::cell::Cell;
use std::io::IsTerminal;

use clifacet::FailureKind;
use clifacet::policy::{Capabilities, Mode, Policy, Request};
use clifacet::prompt::{Choice, Dialog, Prompter, Session};

/// Whether this process can be asked anything at all.
///
/// A dialog that *can* reach a terminal would wait for a person, so the tests that
/// expect a failure skip themselves instead of hanging the suite. All three streams
/// are checked, not just the one the adapter reads: a runner that has any of them
/// attached to a console is a runner where a stray dialog could still block.
fn headless() -> bool {
    !std::io::stdin().is_terminal()
        && !std::io::stdout().is_terminal()
        && !std::io::stderr().is_terminal()
}

/// The failure a dialog on a terminal-less process reports.
fn the_failure_it_reports(
    ask: impl FnOnce() -> Result<(), clifacet::Failure>,
) -> clifacet::Failure {
    ask().expect_err("there is no terminal to ask on")
}

#[test]
fn a_question_without_a_terminal_says_so_instead_of_waiting() {
    if !headless() {
        return;
    }

    let failure = the_failure_it_reports(|| Dialog::new().ask("Which license?").map(drop));

    assert_eq!(
        failure.kind(),
        FailureKind::Execution,
        "the frontend could not run, and nothing asked anything"
    );
    assert!(
        failure.message().contains("terminal"),
        "the message names what is missing: {}",
        failure.message()
    );
}

#[test]
fn a_confirmation_without_a_terminal_fails_the_same_way() {
    if !headless() {
        return;
    }

    let failure = the_failure_it_reports(|| Dialog::new().confirm("Install the hooks?").map(drop));

    assert_eq!(failure.kind(), FailureKind::Execution);
    assert!(
        failure.message().contains("terminal"),
        "{}",
        failure.message()
    );
}

#[test]
fn a_choice_without_a_terminal_fails_the_same_way() {
    if !headless() {
        return;
    }

    let choices = [Choice::new("MIT"), Choice::new("Apache-2.0")];
    let failure =
        the_failure_it_reports(|| Dialog::new().choose("Which license?", &choices).map(drop));

    assert_eq!(failure.kind(), FailureKind::Execution);
    assert!(
        failure.message().contains("terminal"),
        "{}",
        failure.message()
    );
}

#[test]
fn a_secret_without_a_terminal_fails_the_same_way() {
    if !headless() {
        return;
    }

    let failure = the_failure_it_reports(|| Dialog::new().ask_secret("API token?").map(drop));

    assert_eq!(failure.kind(), FailureKind::Execution);
    assert!(
        failure.message().contains("terminal"),
        "{}",
        failure.message()
    );
}

#[test]
fn the_dialog_is_what_a_session_asks_through() {
    if !headless() {
        return;
    }

    let plan = Policy::up_to(Mode::Interactive)
        .resolve(Request::none(), Capabilities::terminal())
        .expect("prompts are permitted here");
    let mut session = Session::start(plan, || Box::new(Dialog::new()));

    let failure = session
        .value(None, "How many crates?")
        .expect_err("the question could not be asked");

    assert_eq!(
        failure.kind(),
        FailureKind::Execution,
        "a missing terminal is the frontend's problem, not the caller's input"
    );
}

#[test]
fn a_plan_that_asks_nothing_never_builds_the_dialog() {
    let built = Cell::new(false);
    let plan = Policy::non_interactive()
        .resolve(Request::none(), Capabilities::terminal())
        .expect("the default resolves");

    let mut session = Session::start(plan, || {
        built.set(true);
        Box::new(Dialog::new())
    });

    assert_eq!(
        session
            .value(None, "Which license?")
            .expect("nothing may ask"),
        None,
        "a missing value stays missing instead of blocking"
    );
    assert!(
        !built.get(),
        "a run that may ask nothing must not construct a dialog"
    );
}
