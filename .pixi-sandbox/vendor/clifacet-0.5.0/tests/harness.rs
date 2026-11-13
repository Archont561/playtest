//! The consumer-facing test facilities: measuring a run, recording what it did, and
//! comparing runs from different frontends.
//!
//! Everything here is the public API an application is meant to use, so these tests
//! drive it exactly as an outside crate would — no `pub(crate)` helpers, no access to
//! the internals a harness is supposed to hide.

use std::io::Write as _;

use clifacet::Command;
use clifacet::Failure;
use clifacet::harness::{Effects, Outcome, Parity, Recording, Subject};

/// A command that doubles its input and fails on zero.
struct Doubler;

impl Command for Doubler {
    type Input = usize;
    type Output = usize;

    fn execute(&self, input: Self::Input) -> Result<Self::Output, Failure> {
        if input == 0 {
            return Err(Failure::execution("nothing to double"));
        }
        Ok(input * 2)
    }
}

/// How a recorded input is described to a reader.
type Description = fn(&usize) -> String;

/// One recording of a [`Doubler`], with the handle its effects are read through.
type Recorded = (Recording<Doubler, Description>, Effects);

/// Records the input of a [`Doubler`].
fn recording() -> Recorded {
    Recording::new(Doubler, |input: &usize| format!("doubler({input})"))
}

#[test]
fn a_measured_run_keeps_the_two_streams_apart() {
    let outcome = Outcome::measured(|out, err| {
        writeln!(out, "data line").expect("writing to a buffer cannot fail");
        writeln!(err, "diagnostic line").expect("writing to a buffer cannot fail");
        3
    });

    assert_eq!(outcome.exit(), 3);
    assert_eq!(outcome.data(), "data line\n");
    assert_eq!(
        outcome.diagnostic(),
        "diagnostic line\n",
        "a diagnostic never leaks into the data"
    );
}

#[test]
fn a_measured_run_that_wrote_nothing_is_empty_on_both_streams() {
    let outcome = Outcome::measured(|_, _| 0);

    assert_eq!(outcome.data(), "");
    assert_eq!(outcome.diagnostic(), "");
    assert_eq!(outcome.effects(), Vec::<String>::new());
}

#[test]
fn effects_are_attached_to_the_run_that_produced_them() {
    let outcome = Outcome::measured(|_, _| 0).with_effects(["one", "two"]);

    assert_eq!(outcome.effects(), ["one", "two"]);
}

#[test]
fn a_recording_forwards_the_result_and_records_the_input() {
    let (recording, effects) = recording();

    let output = recording.execute(4).expect("4 doubles");

    assert_eq!(output, 8, "the wrapped command still does its work");
    assert_eq!(effects.recorded(), ["doubler(4)"]);
}

#[test]
fn a_failing_execution_is_recorded_too() {
    let (recording, effects) = recording();

    let failure = recording.execute(0).expect_err("0 cannot be doubled");

    assert_eq!(failure.kind(), clifacet::FailureKind::Execution);
    assert_eq!(
        effects.recorded(),
        ["doubler(0)"],
        "what reached the command is recorded even when the command fails"
    );
}

#[test]
fn an_effects_handle_is_shared_with_the_recording() {
    let (recording, effects) = recording();
    let observer = effects.clone();

    recording.execute(1).expect("1 doubles");

    assert_eq!(observer.recorded(), ["doubler(1)"]);
    assert_eq!(effects.len(), 1);
}

#[test]
fn every_differing_subject_is_named() {
    let left = Outcome::measured(|out, err| {
        writeln!(out, "left").expect("buffer");
        writeln!(err, "left diagnosed").expect("buffer");
        0
    });
    let right = Outcome::measured(|out, err| {
        writeln!(out, "right").expect("buffer");
        writeln!(err, "right diagnosed").expect("buffer");
        2
    })
    .with_effects(["a"]);

    let differences = left.differences(&right);

    for subject in ["exit", "data", "diagnostic", "effects"] {
        assert!(
            differences.iter().any(|line| line.starts_with(subject)),
            "the {subject} difference is reported: {differences:?}"
        );
    }
}

#[test]
fn identical_runs_have_no_differences() {
    let run = || {
        Outcome::measured(|out, _| {
            writeln!(out, "hello").expect("buffer");
            0
        })
        .with_effects(["doubler(1)"])
    };

    assert_eq!(run().differences(&run()), Vec::<String>::new());
}

#[test]
fn parity_reports_agreement_between_modes() {
    let reference = Outcome::measured(|out, _| {
        writeln!(out, "hello").expect("buffer");
        0
    });

    let parity = Parity::new("flags", reference).compare(
        "scripted prompt",
        Outcome::measured(|out, _| {
            writeln!(out, "hello").expect("buffer");
            0
        }),
    );

    assert_eq!(parity.check(), Ok(()));
    parity.assert_parity();
}

#[test]
fn parity_names_the_mode_that_does_not_match() {
    let reference = Outcome::measured(|out, _| {
        writeln!(out, "hello, ada").expect("buffer");
        0
    });
    let parity = Parity::new("flags", reference).compare(
        "headless tui",
        Outcome::measured(|out, _| {
            writeln!(out, "hello, world").expect("buffer");
            0
        }),
    );

    let differences = parity.check().expect_err("the data differs");

    assert!(
        differences.iter().any(|line| line.contains("headless tui")),
        "the report says which mode disagreed: {differences:?}"
    );
}

#[test]
#[should_panic(expected = "the modes do not agree")]
fn assert_parity_panics_with_the_disagreement() {
    let reference = Outcome::measured(|out, _| {
        writeln!(out, "one").expect("buffer");
        0
    });
    let other = Outcome::measured(|out, _| {
        writeln!(out, "two").expect("buffer");
        0
    });

    Parity::new("flags", reference)
        .compare("headless tui", other)
        .assert_parity();
}

#[test]
fn parity_with_no_other_modes_agrees_with_itself() {
    let reference = Outcome::measured(|_, _| 0);

    assert_eq!(Parity::new("flags", reference).check(), Ok(()));
}

#[test]
fn parity_can_be_limited_to_the_subjects_a_scenario_is_about() {
    let reference = Outcome::measured(|out, err| {
        writeln!(out, "same data").expect("buffer");
        writeln!(err, "dismissed by the user").expect("buffer");
        130
    });
    let other = Outcome::measured(|out, err| {
        writeln!(out, "same data").expect("buffer");
        writeln!(err, "the form was cancelled").expect("buffer");
        130
    });

    // The two frontends word a cancellation differently on purpose, so a scenario
    // about the contract compares only the contract.
    let parity = Parity::new("scripted prompt", reference)
        .comparing([Subject::Exit, Subject::Data])
        .compare("headless tui", other);

    assert_eq!(parity.check(), Ok(()));
}

#[test]
fn subject_selection_is_explicit() {
    let outcome = Outcome::measured(|out, _| {
        writeln!(out, "data").expect("buffer");
        7
    });

    assert_eq!(outcome.subject(Subject::Exit), "7");
    assert_eq!(outcome.subject(Subject::Data), "data\n");
    assert_eq!(outcome.subject(Subject::Diagnostic), "");
    assert_eq!(outcome.subject(Subject::Effects), "");
}
