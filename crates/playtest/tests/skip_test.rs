//! Tests for `test_skip!` and `skip_if!` (`Spec 0002 §1`).

use playtest::skip::{should_skip, skipped_count};
use playtest::{skip_if, test_skip};

skip_if!("unix only", cfg!(unix));
skip_if!("never skipped", false);
test_skip!("windows only", "requires a Windows host");

#[test]
fn a_runtime_condition_decides_whether_a_test_skips() {
    assert!(should_skip(true));
    assert!(!should_skip(false));
}

#[test]
fn a_skipped_test_is_recorded_rather_than_silently_passing() {
    // Record in the same process as the assertion: under nextest every test runs in its
    // own process, so a `skip_if!` declared in another test cannot be observed here.
    let before = skipped_count();
    playtest::skip::record("probe", "recorded by this test");
    assert_eq!(skipped_count(), before + 1);
}
