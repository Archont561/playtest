//! The shipped dialog frontend: [`Dialog`], built on `inquire`.
//!
//! [`Prompter`] is the seam a frontend plugs into, and this module is the framework's
//! own second implementation of it: the same questions [`Terminal`] asks, drawn as a
//! dialog with a cursor, a help line and bracketed paste, instead of read as a line of
//! text — a choice drawn as a list to move through, instead of a number to type, and a
//! secret typed blind, which a line-reading terminal cannot offer at all.
//!
//! What it deliberately does *not* do is decide anything. A dialog is still only
//! built when the resolved [`Plan`](crate::policy::Plan) asks the user something, so a
//! piped run asks nothing and a `--yes` run answers confirmations without one — the
//! policy lives in [`Session`](crate::prompt::Session), not here.
//!
//! `inquire` draws on `stderr` and reads keys from the terminal, which is the same
//! split every other path uses: nothing a person sees reaches the data stream.
//!
//! [`Terminal`]: crate::prompt::Terminal

use std::io::IsTerminal;

use inquire::{Confirm, InquireError, Password, PasswordDisplayMode, Select, Text};

use crate::failure::Failure;
use crate::prompt::{Choice, Prompter};

/// A dialog on the terminal, drawn by `inquire`.
///
/// It holds no state: everything a run needs to know is in the [`Plan`] the session
/// was started with, which is why one value can serve every question.
///
/// ```no_run
/// use clifacet::prompt::{Dialog, Prompter};
///
/// // The same seam `Terminal` and a test's `Scripted` implement.
/// let mut frontend = Dialog::new();
/// let answer = frontend.ask("Which license?").expect("a terminal to ask on");
/// # let _ = answer;
/// ```
///
/// [`Plan`]: crate::policy::Plan
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Dialog;

impl Dialog {
    /// A dialog on this process's terminal.
    ///
    /// Constructing one touches nothing: the terminal is only entered when a question
    /// is actually asked, so a plan that turns out to ask nothing costs nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl Prompter for Dialog {
    fn ask(&mut self, question: &str) -> Result<String, Failure> {
        if !can_ask() {
            return Err(no_terminal());
        }
        Text::new(question).prompt().map_err(failure)
    }

    fn confirm(&mut self, question: &str) -> Result<bool, Failure> {
        if !can_ask() {
            return Err(no_terminal());
        }
        // No is the default: a confirmation that nobody answered must not be read as
        // consent, which is also what `Session::confirm` does when nothing may ask.
        Confirm::new(question)
            .with_default(false)
            .prompt()
            .map_err(failure)
    }

    fn choose(&mut self, question: &str, choices: &[Choice]) -> Result<usize, Failure> {
        if !can_ask() {
            return Err(no_terminal());
        }
        select(question, choices)
            .raw_prompt()
            .map(|picked| picked.index)
            .map_err(failure)
    }

    fn ask_secret(&mut self, question: &str) -> Result<String, Failure> {
        if !can_ask() {
            return Err(no_terminal());
        }
        password(question).prompt().map_err(failure)
    }
}

/// The question a secret is asked with: typed blind, asked once.
///
/// Hidden rather than masked, so not even its length is on screen, and without the
/// toggle that would reveal it. There is no "type it again" field: that guards a
/// secret being *chosen*, and this asks for one that already exists.
fn password(question: &str) -> Password<'_> {
    Password::new(question)
        .with_display_mode(PasswordDisplayMode::Hidden)
        .without_confirmation()
}

/// The selectable list a choice is drawn as: one line per choice, in the order offered.
///
/// `inquire` reports the position of the line picked in this original order, even
/// when the list was filtered by typing, so the position maps straight back to the
/// choice without matching labels.
fn select<'a>(question: &'a str, choices: &[Choice]) -> Select<'a, String> {
    Select::new(question, choices.iter().map(Choice::line).collect())
}

/// Whether this process has an input device a dialog can read keys from.
///
/// Checked before `inquire` is called, rather than only trusting the error it
/// reports. `inquire` turns a failed raw-mode switch into [`InquireError::NotTTY`],
/// but *how* raw mode fails is the platform's business: on Windows a redirected
/// stdin does not always produce the error code that mapping looks for, and the
/// prompt can then wait for a key that will never arrive. One `isatty` on stdin is
/// the cheap, portable answer, and it makes "no terminal" a reported failure on
/// every platform instead of a hang on some.
fn can_ask() -> bool {
    std::io::stdin().is_terminal()
}

/// The failure a dialog reports when there is nothing to read keys from.
fn no_terminal() -> Failure {
    Failure::execution("the dialog needs a terminal, and this run has none")
}

/// Turns a dialog error into the failure category it means.
///
/// The distinction the caller acts on is cancellation: pressing Escape or Ctrl-C is a
/// decision, and it costs its own exit code, while a dialog that cannot run at all is
/// the frontend breaking. Everything else — no terminal, an unusable configuration, an
/// I/O error — is the second kind, and none of them are the caller's input.
fn failure(error: InquireError) -> Failure {
    match error {
        InquireError::OperationCanceled => Failure::cancelled("the dialog was dismissed"),
        InquireError::OperationInterrupted => Failure::cancelled("the dialog was interrupted"),
        InquireError::NotTTY => no_terminal(),
        other => Failure::execution(format!("the dialog could not run: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use inquire::InquireError;

    use inquire::PasswordDisplayMode;

    use super::{failure, password, select};
    use crate::failure::{Failure, FailureKind};
    use crate::prompt::Choice;

    /// Every error `inquire` can report, and the category it means.
    fn cases() -> Vec<(InquireError, FailureKind)> {
        vec![
            (InquireError::OperationCanceled, FailureKind::Cancelled),
            (InquireError::OperationInterrupted, FailureKind::Cancelled),
            (InquireError::NotTTY, FailureKind::Execution),
            (
                InquireError::InvalidConfiguration(String::from("no help field")),
                FailureKind::Execution,
            ),
            (
                InquireError::IO(io::Error::other("the terminal went away")),
                FailureKind::Execution,
            ),
        ]
    }

    #[test]
    fn a_dialog_error_keeps_its_category() {
        for (error, expected) in cases() {
            let reported: Failure = failure(error);

            assert_eq!(reported.kind(), expected, "{}", reported.message());
        }
    }

    /// A choice is drawn as `inquire`'s selectable list: every label, in the order it
    /// was offered, with its help beside it, and the cursor on the first.
    #[test]
    fn a_choice_is_offered_as_a_selectable_list_of_its_labels() {
        let choices = [
            Choice::new("to do"),
            Choice::new("done").with_help("finished and verified"),
        ];

        let offered = select("Which status?", &choices);

        assert_eq!(offered.message, "Which status?");
        assert_eq!(offered.options, ["to do", "done — finished and verified"]);
        assert_eq!(offered.starting_cursor, 0);
    }

    /// A secret is typed blind — not even masked, so its length does not show — and
    /// asked once: a second "confirm it" field belongs to choosing a new secret, not to
    /// supplying an existing one.
    #[test]
    fn a_secret_is_asked_hidden_and_once() {
        let asked = password("API token?");

        assert_eq!(asked.message, "API token?");
        assert_eq!(asked.display_mode, PasswordDisplayMode::Hidden);
        assert!(!asked.enable_confirmation);
        assert!(
            !asked.enable_display_toggle,
            "it cannot be revealed on screen"
        );
    }

    #[test]
    fn a_missing_terminal_is_explained_rather_than_reported_as_a_bare_error() {
        let reported: Failure = failure(InquireError::NotTTY);

        assert!(
            reported.message().contains("terminal"),
            "the message names the missing thing: {}",
            reported.message()
        );
    }
}
