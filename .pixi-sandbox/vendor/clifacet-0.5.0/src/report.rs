//! Writing what went wrong, and answering with what it costs.
//!
//! Every application ends the same way: a [`Failure`] is written on the diagnostic
//! stream and costs what its kind costs, and `clap`'s own verdict on the invocation is
//! written where `clap` says — help and the version on the data stream as a success, a
//! usage error with its usage on the diagnostic stream as a usage failure. [`render`]
//! is that mapping, once, against the application's [`ExitPolicy`], so the numbers stay
//! the application's (decision-3) while the code that applies them stops being
//! rewritten in every `main`.
//!
//! ```
//! use clifacet::Failure;
//! use clifacet::contract::ExitPolicy;
//! use clifacet::report;
//!
//! let policy = ExitPolicy::standard();
//! let (mut out, mut err) = (Vec::new(), Vec::new());
//!
//! let code = report::render(
//!     "greet",
//!     &Failure::validation("--name must not be blank"),
//!     &policy,
//!     &mut out,
//!     &mut err,
//! );
//!
//! assert_eq!(code, policy.validation());
//! assert_eq!(String::from_utf8(err).unwrap(), "greet: --name must not be blank\n");
//! assert!(out.is_empty());
//! ```
//!
//! A failure is written as `program: message`. Its kind is not repeated in the text:
//! the exit code carries it, and the message is already written for a person.

use std::io::Write;

use crate::contract::ExitPolicy;
use crate::failure::Failure;

/// Something [`render`] can write: a failed run, or `clap`'s answer to the invocation.
///
/// Built from either with `From`, so a call site passes `&failure` or `&error` as it
/// has it.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum Report<'a> {
    /// A run that failed validation or execution, or that the user cancelled.
    Failure(&'a Failure),
    /// `clap`'s verdict instead of a parse: a usage error, or the help or version
    /// that was asked for.
    Clap(&'a clap::Error),
}

impl<'a> From<&'a Failure> for Report<'a> {
    fn from(failure: &'a Failure) -> Self {
        Self::Failure(failure)
    }
}

impl<'a> From<&'a clap::Error> for Report<'a> {
    fn from(error: &'a clap::Error) -> Self {
        Self::Clap(error)
    }
}

/// Writes `report` on the stream it belongs on and answers with the code it costs.
///
/// - A [`Failure`] is written as `program: message` on `err`, and costs
///   [`ExitPolicy::for_failure`].
/// - A `clap` usage error is written as `clap` renders it — its message, the usage
///   line and the hint to try `--help` — on `err`, and costs [`ExitPolicy::usage`].
/// - Help and the version are the output that was asked for: written on `out`, and
///   costing [`ExitPolicy::success`].
///
/// A stream that cannot be written — a closed pipe, usually — is ignored: the code is
/// the answer either way, and failing to report a failure has nowhere left to go.
pub fn render<'a>(
    program: &str,
    report: impl Into<Report<'a>>,
    policy: &ExitPolicy,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match report.into() {
        Report::Failure(failure) => {
            let _ = writeln!(err, "{program}: {}", failure.message());
            policy.for_failure(failure)
        }
        // `clap` decides the stream as well as the outcome: help is the result the
        // caller asked for, while a usage error is a diagnostic.
        Report::Clap(error) if error.use_stderr() => {
            let _ = write!(err, "{error}");
            policy.usage()
        }
        Report::Clap(error) => {
            let _ = write!(out, "{error}");
            policy.success()
        }
    }
}
