//! One place that writes what went wrong and answers with what it costs
//! (decision-18 item 7).
//!
//! Every application `main` used to rewrite the same mapping: a failure on the
//! diagnostic stream with the code its kind costs, `clap`'s help on the data stream,
//! and `clap`'s usage error on the diagnostic stream with the usage code.

use clap::Parser;
use clifacet::contract::ExitPolicy;
use clifacet::report;
use clifacet::{Failure, FailureKind};
use rstest::rstest;

const POLICY: ExitPolicy = ExitPolicy::standard();

#[derive(Debug, Parser)]
#[command(name = "demo", version = "1.2.3", about = "A demonstration")]
struct Demo {
    /// Who to greet.
    #[arg(long)]
    name: Option<String>,
}

/// Renders `report` and answers with the code and both streams.
fn rendered<'a>(report: impl Into<report::Report<'a>>) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = report::render("demo", report, &POLICY, &mut out, &mut err);
    (
        code,
        String::from_utf8(out).expect("utf-8"),
        String::from_utf8(err).expect("utf-8"),
    )
}

#[rstest]
#[case::validation(Failure::validation("--name must not be blank"))]
#[case::execution(Failure::execution("the operation failed"))]
#[case::cancelled(Failure::cancelled("the user declined"))]
fn a_failure_is_a_diagnostic_that_costs_what_its_kind_costs(#[case] failure: Failure) {
    let (code, out, err) = rendered(&failure);

    assert_eq!(err, format!("demo: {}\n", failure.message()));
    assert!(
        out.is_empty(),
        "a failure never reaches the data stream: {out}"
    );
    assert_eq!(code, POLICY.for_kind(failure.kind()));
}

#[test]
fn the_kinds_cost_different_codes() {
    let (validation, ..) = rendered(&Failure::validation("x"));
    let (execution, ..) = rendered(&Failure::execution("x"));
    let (cancelled, ..) = rendered(&Failure::cancelled("x"));

    assert_eq!(validation, POLICY.for_kind(FailureKind::Validation));
    assert_ne!(validation, execution);
    assert_ne!(execution, cancelled);
}

#[test]
fn a_usage_error_is_a_diagnostic_with_the_usage_and_the_usage_code() {
    let error = Demo::try_parse_from(["demo", "--bogus"]).expect_err("an unknown flag");

    let (code, out, err) = rendered(&error);

    assert!(err.contains("--bogus"), "clap's own words: {err}");
    assert!(err.contains("Usage: demo"), "the usage is added: {err}");
    assert!(out.is_empty(), "{out}");
    assert_eq!(code, POLICY.usage());
}

#[rstest]
#[case::help(&["demo", "--help"], "A demonstration")]
#[case::version(&["demo", "--version"], "demo 1.2.3")]
fn help_and_the_version_are_the_data_that_was_asked_for(
    #[case] argv: &[&str],
    #[case] expected: &str,
) {
    let error = Demo::try_parse_from(argv).expect_err("clap answers instead of parsing");

    let (code, out, err) = rendered(&error);

    assert!(out.contains(expected), "{out}");
    assert!(err.is_empty(), "{err}");
    assert_eq!(code, POLICY.success());
}
