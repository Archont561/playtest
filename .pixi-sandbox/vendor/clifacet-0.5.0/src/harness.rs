//! Proving that the mode which collected input changed nothing else.
//!
//! decision-1 makes parity a property to be exercised at public seams, and decision-6
//! makes scripted and headless adapters the way to exercise it. This module is that
//! property, published: an application can measure a run, record what its command
//! received, and compare runs that came from different frontends.
//!
//! Three pieces, and nothing else:
//!
//! - [`Outcome`] is one run, reduced to the four things that can differ: the exit
//!   code, the data stream, the diagnostic stream, and the effects the run had.
//! - [`Recording`] wraps a [`Command`](crate::Command) and records the input each
//!   execution received, so "the same answer" is an assertion rather than a guess.
//! - [`Parity`] compares runs and reports which one disagreed, and about what.
//!
//! ```no_run
//! use std::io::Write as _;
//!
//! use clifacet::harness::{Outcome, Parity, Subject};
//!
//! // Any of the application's entry points: flags, a scripted prompt, or the form.
//! let flags = Outcome::measured(|out, _err| {
//!     writeln!(out, "hello, ada").expect("writing to a buffer");
//!     0
//! });
//! let prompted = Outcome::measured(|out, _err| {
//!     writeln!(out, "hello, ada").expect("writing to a buffer");
//!     0
//! });
//!
//! Parity::new("flags", flags)
//!     .comparing([Subject::Exit, Subject::Data])
//!     .compare("scripted prompt", prompted)
//!     .assert_parity();
//! ```
//!
//! There is no mode enum and no runner trait on purpose. A frontend is how an
//! application's own entry point is called, and the harness only needs the result;
//! requiring an application to describe its frontends to the library would be the
//! global registration decision-4 rules out.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;

use crate::command::Command;

/// One run of an application, reduced to what a comparison can disagree about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    exit: i32,
    data: String,
    diagnostic: String,
    effects: Vec<String>,
}

impl Outcome {
    /// Runs `run` against captured streams and records what it produced.
    ///
    /// The closure receives the data stream and the diagnostic stream, in that order.
    /// Either may be written to freely: they are buffers, so a run that touches a
    /// terminal is not a run this can measure.
    pub fn measured(run: impl FnOnce(&mut Vec<u8>, &mut Vec<u8>) -> i32) -> Self {
        let (mut data, mut diagnostic) = (Vec::new(), Vec::new());
        let exit = run(&mut data, &mut diagnostic);

        Self {
            exit,
            data: String::from_utf8_lossy(&data).into_owned(),
            diagnostic: String::from_utf8_lossy(&diagnostic).into_owned(),
            effects: Vec::new(),
        }
    }

    /// Attaches what the run's command received.
    ///
    /// An [`Effects`] handle belongs to one run, so this takes the recorded values
    /// rather than the handle: a run cannot borrow another run's evidence.
    #[must_use]
    pub fn with_effects<I, S>(mut self, effects: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.effects = effects.into_iter().map(Into::into).collect();
        self
    }

    /// The process status the run answered with.
    #[must_use]
    pub const fn exit(&self) -> i32 {
        self.exit
    }

    /// Everything the run wrote to its data stream.
    #[must_use]
    pub fn data(&self) -> &str {
        &self.data
    }

    /// Everything the run wrote to its diagnostic stream.
    #[must_use]
    pub fn diagnostic(&self) -> &str {
        &self.diagnostic
    }

    /// What the run's command received, in order.
    #[must_use]
    pub fn effects(&self) -> &[String] {
        &self.effects
    }

    /// One subject as text, for reporting.
    #[must_use]
    pub fn subject(&self, subject: Subject) -> String {
        match subject {
            Subject::Exit => self.exit.to_string(),
            Subject::Data => self.data.clone(),
            Subject::Diagnostic => self.diagnostic.clone(),
            Subject::Effects => self.effects.join(", "),
        }
    }

    /// Every subject on which this run and `other` disagree.
    ///
    /// One line per difference, each starting with the subject's name, so a failing
    /// comparison reads as a list rather than as the first assertion that tripped.
    #[must_use]
    pub fn differences(&self, other: &Self) -> Vec<String> {
        let mut differences = Vec::new();
        for subject in Subject::ALL {
            let (left, right) = (self.subject(subject), other.subject(subject));
            if left != right {
                let mut line = String::new();
                // Writing into a `String` cannot fail; ignoring the result keeps the
                // report from turning into a panic about formatting.
                let _ = write!(line, "{}: {left:?} is not {right:?}", subject.name());
                differences.push(line);
            }
        }
        differences
    }
}

/// One thing two runs can be compared about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subject {
    /// The exit code.
    Exit,
    /// The data stream.
    Data,
    /// The diagnostic stream.
    Diagnostic,
    /// What the command received.
    Effects,
}

impl Subject {
    /// Every subject, in the order differences are reported.
    pub const ALL: [Self; 4] = [Self::Exit, Self::Data, Self::Diagnostic, Self::Effects];

    /// How the subject is named in a report.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Exit => "exit",
            Self::Data => "data",
            Self::Diagnostic => "diagnostic",
            Self::Effects => "effects",
        }
    }
}

/// A comparison of runs that should have produced the same result.
///
/// The first run is the reference; every [`Parity::compare`] adds another. Nothing is
/// asserted until [`Parity::check`] or [`Parity::assert_parity`], so a caller can
/// gather every mode's outcome first and report all the differences at once.
#[derive(Debug, Clone)]
pub struct Parity {
    subjects: Vec<Subject>,
    reference: (String, Outcome),
    others: Vec<(String, Outcome)>,
}

impl Parity {
    /// Starts a comparison against `reference`, labelled with the mode it came from.
    #[must_use]
    pub fn new(label: impl Into<String>, reference: Outcome) -> Self {
        Self {
            subjects: Subject::ALL.to_vec(),
            reference: (label.into(), reference),
            others: Vec::new(),
        }
    }

    /// Compares only these subjects.
    ///
    /// Frontends are allowed to word a cancellation differently while agreeing on the
    /// contract, so a scenario about the contract names the subjects it means.
    #[must_use]
    pub fn comparing(mut self, subjects: impl IntoIterator<Item = Subject>) -> Self {
        self.subjects = subjects.into_iter().collect();
        self
    }

    /// Adds another mode's outcome.
    #[must_use]
    pub fn compare(mut self, label: impl Into<String>, outcome: Outcome) -> Self {
        self.others.push((label.into(), outcome));
        self
    }

    /// Every disagreement, each naming the mode that disagreed.
    ///
    /// # Errors
    ///
    /// Returns the differences — one line per subject per disagreeing mode — when the
    /// runs do not agree. An empty result means they do.
    pub fn check(&self) -> Result<(), Vec<String>> {
        let (reference_label, reference) = &self.reference;

        let mut differences = Vec::new();
        for (label, outcome) in &self.others {
            for subject in &self.subjects {
                let (left, right) = (reference.subject(*subject), outcome.subject(*subject));
                if left != right {
                    differences.push(format!(
                        "{label} differs from {reference_label} on {}: {left:?} is not {right:?}",
                        subject.name()
                    ));
                }
            }
        }

        if differences.is_empty() {
            Ok(())
        } else {
            Err(differences)
        }
    }

    /// Panics with every difference when the runs do not agree.
    ///
    /// This is the assertion shape a test wants; [`Parity::check`] is the same result
    /// for a caller that would rather report than unwind.
    ///
    /// # Panics
    ///
    /// Panics when the runs disagree, listing every difference.
    pub fn assert_parity(&self) {
        if let Err(differences) = self.check() {
            panic!("the modes do not agree:\n{}", differences.join("\n"));
        }
    }
}

/// A handle to what one [`Recording`] has seen.
///
/// Cloning shares the record, so a test can keep the handle while the wrapped command
/// is moved into the application.
#[derive(Debug, Clone, Default)]
pub struct Effects {
    recorded: Rc<RefCell<Vec<String>>>,
}

impl Effects {
    /// What the command received, in order.
    #[must_use]
    pub fn recorded(&self) -> Vec<String> {
        self.recorded.borrow().clone()
    }

    /// How many executions reached the command.
    #[must_use]
    pub fn len(&self) -> usize {
        self.recorded.borrow().len()
    }

    /// Whether the command was never reached.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.recorded.borrow().is_empty()
    }
}

/// A command that records what it is asked to run, then runs it.
///
/// The record is kept even when the command fails: "what reached the command" is a
/// fact about the run either way, and a failure that recorded nothing looks the same
/// as a run that never executed.
#[derive(Debug, Clone)]
pub struct Recording<C, F> {
    inner: C,
    effects: Effects,
    describe: F,
}

impl<C, F> Recording<C, F>
where
    C: Command,
    F: Fn(&C::Input) -> String,
{
    /// Wraps `command`, describing each input with `describe`.
    ///
    /// The description is the caller's because only the application knows which parts
    /// of its input matter for a comparison — and because requiring `Debug` on an
    /// application's input type would be a rule about their types, not about parity.
    pub fn new(command: C, describe: F) -> (Self, Effects) {
        let effects = Effects::default();
        let recording = Self {
            inner: command,
            effects: effects.clone(),
            describe,
        };
        (recording, effects)
    }
}

impl<C, F> Command for Recording<C, F>
where
    C: Command,
    F: Fn(&C::Input) -> String,
{
    type Input = C::Input;
    type Output = C::Output;

    fn execute(&self, input: Self::Input) -> Result<Self::Output, crate::failure::Failure> {
        self.effects
            .recorded
            .borrow_mut()
            .push((self.describe)(&input));
        self.inner.execute(input)
    }
}
