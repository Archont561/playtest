//! Library-owned failure categories for the UI-free command core.
//!
//! A command can fail in three places, and the difference matters to a caller: the
//! input can be rejected before the command ever runs ([`Failure::validation`]),
//! the command can run and fail ([`Failure::execution`]), or the user can stop an
//! interactive flow before it produces input at all ([`Failure::cancelled`]). All
//! three travel as one type so every presentation layer — flags today, prompts and
//! a TUI later — reports them the same way, and so an application cannot mistake a
//! cancellation for bad input.
//!
//! Exit codes are deliberately *not* here. decision-3 keeps output and exit
//! mapping application-owned, so this type answers "what kind of failure is
//! this?" and the application decides what that costs.

use std::error::Error;
use std::fmt;

/// What kind of failure a [`Failure`] records.
///
/// With the `serde` feature it serialises as its lowercase name: `"validation"`,
/// `"execution"` or `"cancelled"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize),
    serde(rename_all = "lowercase")
)]
#[non_exhaustive]
pub enum FailureKind {
    /// The input failed validation, so the command did not run.
    Validation,
    /// The command ran and could not complete.
    Execution,
    /// The user stopped an interactive flow before it produced input.
    ///
    /// Nothing ran. Kept apart from [`FailureKind::Validation`] because the input
    /// was never wrong — the user changed their mind, which is a decision rather
    /// than a mistake, and deserves its own message and exit code.
    Cancelled,
}

/// A command failure, with the kind and a caller-facing message.
///
/// The message is written for a person reading a diagnostic, not for parsing.
/// Machine-readable output belongs on the application's data stream, which a
/// failure never writes to.
///
/// With the `serde` feature a failure serialises as its kind and its message, in that
/// order — `{"kind":"validation","message":"--name must not be blank"}` through
/// `serde_json` — which is the body an application writes when it reports failures as
/// JSON. The library never writes it itself: output stays the application's
/// (decision-3), so the flag that selects JSON and the stream it goes to are too.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Failure {
    kind: FailureKind,
    message: String,
}

impl Failure {
    /// Records that the input failed validation.
    ///
    /// The command was never invoked, so nothing it owns has changed.
    #[must_use]
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            kind: FailureKind::Validation,
            message: message.into(),
        }
    }

    /// Records that the command ran and could not complete.
    #[must_use]
    pub fn execution(message: impl Into<String>) -> Self {
        Self {
            kind: FailureKind::Execution,
            message: message.into(),
        }
    }

    /// Records that the user cancelled before anything ran.
    ///
    /// A frontend that is dismissed, or a confirmation the user answers with
    /// "no", reports this instead of pretending the input was invalid.
    #[must_use]
    pub fn cancelled(message: impl Into<String>) -> Self {
        Self {
            kind: FailureKind::Cancelled,
            message: message.into(),
        }
    }

    /// The category of this failure.
    ///
    /// Applications map this to their own exit codes and messages, which is what
    /// keeps the numbers out of the library; see the `clifacet-example` crate for a worked
    /// mapping that gives each kind its own code.
    #[must_use]
    pub const fn kind(&self) -> FailureKind {
        self.kind
    }

    /// The caller-facing message, without the diagnostic prefix an application adds.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for Failure {}
