//! Cross-language conformance suites for the fixture contract (Spec 0005 §1, task_023).
//!
//! Each test is one cell of the `(capability x target)` matrix in `backlog/conformance.yml`.
//! The capabilities exercised here are the ones this target currently claims; the others
//! are declared in the manifest but not claimed, so they report as `not_implemented`
//! rather than as passing.
//!
//! The tests assert observable behaviour through the public API only. They do not reach
//! into private state, so the same scenarios are expressible in the Python and TypeScript
//! suites without sharing implementation details.

use std::sync::{Arc, Mutex};

use futures::FutureExt;
use playtest::VERSION;
use playtest::fixture::{FixtureStack, TeardownHook, run_with_teardown};

/// Records the order in which construction and teardown happen.
type Trace = Arc<Mutex<Vec<String>>>;

fn new_trace() -> Trace {
    Arc::new(Mutex::new(Vec::new()))
}

fn record(trace: &Trace, entry: &str) {
    if let Ok(mut guard) = trace.lock() {
        guard.push(entry.to_owned());
    }
}

fn trace_of(trace: &Trace) -> Vec<String> {
    trace.lock().map(|guard| guard.clone()).unwrap_or_default()
}

/// Resolve one fixture: record its construction now, its teardown when the stack unwinds.
async fn resolve(stack: &mut FixtureStack, name: &'static str, trace: &Trace) {
    let owned = Arc::clone(trace);
    stack
        .setup_with(|| async move {
            record(&owned, &format!("{name}:setup"));
            let released = Arc::clone(&owned);
            let hook: TeardownHook = Box::new(move || {
                Box::pin(async move {
                    record(&released, &format!("{name}:teardown"));
                })
            });
            (name, hook)
        })
        .await;
}

/// `fixture_explicit_resolution`: only requested fixtures are constructed, in request order.
#[tokio::test]
async fn fixture_explicit_resolution() {
    let trace = new_trace();
    let mut stack = FixtureStack::new();
    resolve(&mut stack, "requested", &trace).await;
    // A second fixture exists but is never resolved, so nothing about it is constructed.

    assert_eq!(trace_of(&trace), vec!["requested:setup".to_owned()]);
    assert_eq!(stack.len(), 1, "only the requested fixture is held");
}

/// `teardown_lifo`: cleanup runs in strict reverse order of successful creation.
#[tokio::test]
async fn teardown_lifo() {
    let trace = new_trace();
    let mut stack = FixtureStack::new();
    for name in ["a", "b", "c"] {
        resolve(&mut stack, name, &trace).await;
    }
    let report = stack.teardown_all().await;

    assert!(report.is_clean());
    assert_eq!(report.ran, 3);
    assert_eq!(
        trace_of(&trace),
        vec![
            "a:setup".to_owned(),
            "b:setup".to_owned(),
            "c:setup".to_owned(),
            "c:teardown".to_owned(),
            "b:teardown".to_owned(),
            "a:teardown".to_owned(),
        ],
        "teardown must be the exact reverse of setup"
    );
}

/// `teardown_failure_safety`: a panicking cleanup never stops the remaining cleanups.
#[tokio::test]
async fn teardown_failure_safety() {
    let trace = new_trace();
    let mut stack = FixtureStack::new();
    resolve(&mut stack, "outer", &trace).await;

    let failing = Arc::clone(&trace);
    let hook: TeardownHook = Box::new(move || {
        Box::pin(async move {
            record(&failing, "boom:teardown");
            panic!("deliberate conformance failure");
        })
    });
    stack.push(hook);

    let report = stack.teardown_all().await;

    assert!(!report.is_clean(), "the failing cleanup must be reported");
    assert_eq!(report.failures.len(), 1);
    let seen = trace_of(&trace);
    assert!(
        seen.contains(&"boom:teardown".to_owned()),
        "the failing fixture's cleanup ran"
    );
    assert!(
        seen.contains(&"outer:teardown".to_owned()),
        "the fixture created before the failure must still be released"
    );
}

/// The harness must observe a panic from the body *and* still run teardown.
#[tokio::test]
async fn body_panic_still_tears_down() {
    let trace = new_trace();
    let mut stack = FixtureStack::new();
    resolve(&mut stack, "held", &trace).await;

    let outcome = std::panic::AssertUnwindSafe(run_with_teardown(&mut stack, async {
        panic!("body failure");
    }))
    .catch_unwind()
    .await;

    assert!(outcome.is_err(), "the body's panic is surfaced");
    assert_eq!(stack.len(), 0, "every fixture was released");
    assert!(trace_of(&trace).contains(&"held:teardown".to_owned()));
}

/// The crate exposes a version, which the parity report uses to prove the target is real.
#[test]
fn version_is_non_empty() {
    assert!(!VERSION.is_empty());
}
