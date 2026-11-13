//! Skip reporting for `test_skip!` and `skip_if!` (`Spec 0002 §1`).
//!
//! A skipped test must be distinguishable from a passing one. The library therefore
//! returns a [`Skip`] record describing what happened, and the generated test function
//! prints it: reporting output belongs to the test harness, not to this crate
//! (`PCC-S01`, structured output in library code).

use std::sync::atomic::{AtomicUsize, Ordering};

/// What a skip declaration decided, and why (`Spec 0002 §1`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skip {
    /// The test name, as declared.
    pub test: String,
    /// Why the test did not run.
    pub reason: String,
}

impl Skip {
    /// A skip with its reason.
    #[must_use]
    pub fn new(test: &str, reason: &str) -> Self {
        Self {
            test: test.to_owned(),
            reason: reason.to_owned(),
        }
    }

    /// The line a report parser recognizes.
    #[must_use]
    pub fn report_line(&self) -> String {
        format!("SKIPPED {}: {}", self.test, self.reason)
    }
}

/// Every skip the current process has recorded.
static SKIPS: AtomicUsize = AtomicUsize::new(0);

/// How many skips the current process has recorded.
#[must_use]
pub fn skipped_count() -> usize {
    SKIPS.load(Ordering::Relaxed)
}

/// Record a skip and return it, so the caller can report it.
pub fn record(test_name: &str, reason: &str) -> Skip {
    SKIPS.fetch_add(1, Ordering::Relaxed);
    Skip::new(test_name, reason)
}

/// Decide whether a test should run, from the runtime condition of `skip_if!`.
///
/// Kept separate from [`record`] so the decision is testable on its own.
#[must_use]
pub fn should_skip(condition: bool) -> bool {
    condition
}

/// The reason text a conditional skip reports.
#[must_use]
pub fn condition_reason(condition: bool) -> &'static str {
    if condition {
        "the declared condition evaluated to true"
    } else {
        "the declared condition evaluated to false"
    }
}
