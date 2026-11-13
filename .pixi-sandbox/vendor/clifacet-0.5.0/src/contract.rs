//! What a run may write where, and what each outcome costs.
//!
//! The stream half of the contract is a rule rather than a type, because it is a fact
//! about the code: the data stream carries only what a caller asked for, and
//! everything a person sees — a question, a full-screen interface, a diagnostic — goes
//! to the diagnostic stream.
//!
//! | Mode | Data stream | Diagnostic stream |
//! | --- | --- | --- |
//! | flags | the requested data | the failure, if any |
//! | a prompt | the requested data | the questions, then the failure, if any |
//! | the form | the requested data | the screen, then the failure, if any |
//! | help, `--version` | the text that was asked for | — |
//!
//! The exit half is a policy, because the numbers are the application's
//! ([`ExitPolicy`]) while the categories are the library's. [`ExitPolicy::standard`]
//! is the one the reference application uses: success, validation, cancellation and
//! execution each get their own code, and a usage error shares validation's because
//! both mean "nothing ran, fix the invocation".
//!
//! ```
//! use clifacet::Failure;
//! use clifacet::contract::ExitPolicy;
//!
//! let policy = ExitPolicy::standard();
//!
//! assert_eq!(policy.success(), 0);
//! assert_eq!(policy.for_failure(&Failure::validation("bad value")), policy.validation());
//! assert_ne!(policy.validation(), policy.cancelled());
//! ```

use crate::failure::{Failure, FailureKind};

/// The exit codes an application gives its outcomes.
///
/// Build one with [`ExitPolicy::standard`], or with the struct literal syntax when an
/// application needs its own numbers. Nothing else in the library depends on a
/// particular value, which is what keeps decision-3's "the numbers are the
/// application's" true while still making the mapping one readable thing instead of
/// four `match` arms scattered through an application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExitPolicy {
    success: i32,
    usage: i32,
    validation: i32,
    cancelled: i32,
    execution: i32,
}

impl ExitPolicy {
    /// The conventional mapping: `0`, `2` for anything that ran nothing, `130` for a
    /// cancellation and `1` for a run that failed.
    ///
    /// `130` is the shell's own code for "interrupted" (128 + `SIGINT`), and it is
    /// deliberately not `2`: a cancellation is a decision, not a mistake in the
    /// invocation. Validation and usage share `2` because both mean "nothing ran, fix
    /// the invocation", which `clap` documents for its own errors.
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            success: 0,
            usage: 2,
            validation: 2,
            cancelled: 130,
            execution: 1,
        }
    }

    /// The code a completed run answers with.
    #[must_use]
    pub const fn success(&self) -> i32 {
        self.success
    }

    /// The code a rejected invocation answers with — `clap`'s own usage errors
    /// included.
    #[must_use]
    pub const fn usage(&self) -> i32 {
        self.usage
    }

    /// The code an input that failed validation costs.
    #[must_use]
    pub const fn validation(&self) -> i32 {
        self.validation
    }

    /// The code a run the user stopped costs.
    #[must_use]
    pub const fn cancelled(&self) -> i32 {
        self.cancelled
    }

    /// The code a run that could not finish costs.
    #[must_use]
    pub const fn execution(&self) -> i32 {
        self.execution
    }

    /// The code one failure category costs.
    ///
    /// Exhaustive inside this crate because it defines [`FailureKind`]; a consumer
    /// outside it needs a catch-all arm for the same `#[non_exhaustive]` reason.
    #[must_use]
    pub const fn for_kind(&self, kind: FailureKind) -> i32 {
        match kind {
            FailureKind::Validation => self.validation,
            FailureKind::Cancelled => self.cancelled,
            FailureKind::Execution => self.execution,
        }
    }

    /// The code one failure costs.
    #[must_use]
    pub fn for_failure(&self, failure: &Failure) -> i32 {
        self.for_kind(failure.kind())
    }
}
