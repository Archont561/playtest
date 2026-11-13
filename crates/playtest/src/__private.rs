//! Support surface for the macros this crate re-exports.
//!
//! Nothing here is meant to be called by hand. It is `#[doc(hidden)]` so it stays out of
//! the rendered documentation while `playtest-macros` still has a stable path to expand into.

/// Run `future` to completion on a current-thread runtime.
///
/// `test!` expands to a plain `#[test]`, so it has to build its own runtime; the macro
/// cannot depend on the caller's `#[tokio::test]` and still be usable standalone.
pub fn block_on<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("a current-thread runtime is available")
        .block_on(future)
}

use crate::fixture::TeardownReport;

/// Assert that the stack holds exactly the fixtures the test requested.
///
/// A mismatch means the macro bound a different set than it resolved, which would make the
/// "unused fixtures are not constructed" guarantee (Spec 0001 §3.1) unprovable.
pub fn assert_fixture_count(actual: usize, requested: usize) {
    assert_eq!(
        actual, requested,
        "the fixture stack must hold exactly the fixtures the test requested",
    );
}

/// Fail the current test when any fixture teardown panicked.
///
/// Teardown failures are reported after the body's own outcome, so a cleanup bug never
/// hides why the test itself failed (`Spec 0005 §1.1`).
pub fn report_teardown_failures(report: &TeardownReport, test_name: &str) {
    assert!(
        report.is_clean(),
        "{} fixture teardown(s) failed for test {test_name:?}: {}",
        report.failures.len(),
        report.failures.join("; "),
    );
}
