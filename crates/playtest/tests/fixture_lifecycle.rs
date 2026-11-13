//! Fixture lifecycle and teardown-order tests (`Spec 0001 §3`).

use std::sync::{Arc, Mutex};

use futures::FutureExt;
use playtest::fixture::run_with_teardown;
use playtest::{AsyncFixture, FixtureHandle, FixtureStack, TeardownHook};
use rstest::rstest;

/// Records the order in which teardown hooks run.
#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<String>>>);

impl Recorder {
    fn new() -> Self {
        Self::default()
    }

    fn hook(&self, label: &str) -> TeardownHook {
        self.hook_returning(label, ())
    }

    /// A hook for fixture `label` that records its own run and yields `value`.
    fn hook_returning<T: Send + 'static>(&self, label: &str, value: T) -> TeardownHook {
        let recorder = self.clone();
        let label = label.to_owned();
        Box::new(move || {
            let recorder = recorder.clone();
            let label = label.clone();
            let boxed: playtest::BoxFuture<'static, ()> = Box::pin(async move {
                recorder.record(&label);
                drop(value);
            });
            boxed
        })
    }

    fn record(&self, label: &str) {
        if let Ok(mut log) = self.0.lock() {
            log.push(label.to_owned());
        }
    }

    fn seen(&self) -> Vec<String> {
        self.0.lock().map(|log| log.clone()).unwrap_or_default()
    }
}

/// A fixture whose setup is observable, for the [`AsyncFixture`] path.
struct Audited;

impl AsyncFixture for Audited {
    type Output = u32;

    fn setup<'a>() -> playtest::BoxFuture<'a, (Self::Output, TeardownHook)> {
        Box::pin(async {
            let hook: TeardownHook = Box::new(|| {
                let boxed: playtest::BoxFuture<'static, ()> = Box::pin(async {});
                boxed
            });
            (7, hook)
        })
    }
}

/// A fixture whose teardown panics, for the failure-isolation case.
struct Exploding;

impl AsyncFixture for Exploding {
    type Output = ();

    fn setup<'a>() -> playtest::BoxFuture<'a, (Self::Output, TeardownHook)> {
        Box::pin(async {
            let hook: TeardownHook = Box::new(|| {
                let boxed: playtest::BoxFuture<'static, ()> = Box::pin(async {
                    panic!("teardown exploded");
                });
                boxed
            });
            ((), hook)
        })
    }
}

#[tokio::test]
async fn teardown_runs_in_reverse_resolution_order() {
    let recorder = Recorder::new();
    let mut stack = FixtureStack::new();

    for label in ["a", "b", "c"] {
        stack
            .setup_with(|| {
                let hook = recorder.hook(label);
                async move { (label.to_owned(), hook) }
            })
            .await;
    }

    let report = stack.teardown_all().await;

    assert_eq!(report.ran, 3);
    assert!(report.is_clean());
    assert_eq!(recorder.seen(), vec!["c", "b", "a"]);
    assert!(stack.is_empty());
}

#[tokio::test]
async fn setup_resolves_only_the_fixtures_a_test_requests() {
    let mut stack = FixtureStack::new();

    let value = stack.setup::<Audited>().await;

    assert_eq!(value, 7);
    assert_eq!(stack.len(), 1, "only the requested fixture is constructed");
}

#[tokio::test]
async fn fixture_handle_derefs_to_its_output() {
    let recorder = Recorder::new();
    let mut handle = FixtureHandle::new(String::from("alpha"), recorder.hook("h"));

    assert_eq!(handle.as_str(), "alpha");
    handle.push_str("-beta");
    assert_eq!(handle.as_str(), "alpha-beta");
    assert!(handle.is_armed());

    assert!(handle.teardown().await);
    assert!(!handle.is_armed());
    assert_eq!(recorder.seen(), vec!["h"]);
}

#[tokio::test]
async fn fixture_handle_teardown_is_idempotent() {
    let recorder = Recorder::new();
    let mut handle = FixtureHandle::new((), recorder.hook("once"));

    assert!(handle.teardown().await);
    assert!(!handle.teardown().await, "a second teardown is a no-op");
    assert_eq!(recorder.seen(), vec!["once"]);
}

#[tokio::test]
async fn teardown_runs_when_the_test_body_panics() {
    let recorder = Recorder::new();
    let mut stack = FixtureStack::new();
    stack
        .setup_with(|| {
            let hook = recorder.hook("a");
            async move { ((), hook) }
        })
        .await;
    stack
        .setup_with(|| {
            let hook = recorder.hook("b");
            async move { ((), hook) }
        })
        .await;

    let body = async {
        panic!("test body exploded");
    };
    let outcome = std::panic::AssertUnwindSafe(run_with_teardown(&mut stack, body))
        .catch_unwind()
        .await;

    assert!(
        outcome.is_err(),
        "the body's panic is resumed, not swallowed"
    );
    assert_eq!(
        recorder.seen(),
        vec!["b", "a"],
        "teardown still ran, in LIFO order"
    );
    assert!(stack.is_empty());
}

#[tokio::test]
async fn a_panicking_teardown_does_not_stop_the_remaining_hooks() {
    let recorder = Recorder::new();
    let mut stack = FixtureStack::new();

    stack
        .setup_with(|| {
            let hook = recorder.hook("first");
            async move { ((), hook) }
        })
        .await;
    stack.setup::<Exploding>().await;
    stack
        .setup_with(|| {
            let hook = recorder.hook("last");
            async move { ((), hook) }
        })
        .await;

    let report = stack.teardown_all().await;

    assert_eq!(report.ran, 3);
    assert_eq!(report.failures, vec!["teardown exploded"]);
    assert_eq!(
        recorder.seen(),
        vec!["last", "first"],
        "both surviving hooks ran despite the panic between them"
    );
}

/// A fixture whose teardown panics with a `String` payload, not a `&str`.
struct ExplodingWithString;

impl AsyncFixture for ExplodingWithString {
    type Output = ();

    fn setup<'a>() -> playtest::BoxFuture<'a, (Self::Output, TeardownHook)> {
        Box::pin(async {
            let hook: TeardownHook = Box::new(|| {
                let boxed: playtest::BoxFuture<'static, ()> = Box::pin(async {
                    panic!("{}", String::from("owned payload"));
                });
                boxed
            });
            ((), hook)
        })
    }
}

/// A fixture whose teardown panics with a payload that is not a string at all.
struct ExplodingWithOpaquePayload;

impl AsyncFixture for ExplodingWithOpaquePayload {
    type Output = ();

    fn setup<'a>() -> playtest::BoxFuture<'a, (Self::Output, TeardownHook)> {
        Box::pin(async {
            let hook: TeardownHook = Box::new(|| {
                let boxed: playtest::BoxFuture<'static, ()> =
                    Box::pin(async { std::panic::panic_any(42_u8) });
                boxed
            });
            ((), hook)
        })
    }
}

#[tokio::test]
async fn a_string_panic_payload_is_reported_verbatim() {
    let mut stack = FixtureStack::new();
    stack.setup::<ExplodingWithString>().await;

    let report = stack.teardown_all().await;

    assert_eq!(report.failures, vec!["owned payload"]);
}

#[tokio::test]
async fn a_non_string_panic_payload_gets_a_stable_description() {
    let mut stack = FixtureStack::new();
    stack.setup::<ExplodingWithOpaquePayload>().await;

    let report = stack.teardown_all().await;

    assert_eq!(
        report.failures,
        vec!["fixture teardown panicked with a non-string payload"]
    );
}

#[tokio::test]
async fn dropping_an_armed_handle_inside_a_runtime_spawns_its_teardown() {
    // `Drop` cannot await, so an armed hook is spawned onto the ambient runtime. This is
    // the fallback path; waiting for the recorded run proves the spawn actually happened.
    let recorder = Recorder::new();
    {
        let handle = FixtureHandle::new((), recorder.hook_returning("dropped", ()));

        drop(handle);
    }

    for _ in 0..100 {
        if !recorder.seen().is_empty() {
            break;
        }
        tokio::task::yield_now().await;
    }

    assert_eq!(
        recorder.seen(),
        vec!["dropped"],
        "the armed hook was spawned on drop"
    );
}

#[rstest]
#[case(1)]
#[case(4)]
#[case(9)]
#[tokio::test]
async fn teardown_order_holds_for_any_fixture_count(#[case] count: usize) {
    let recorder = Recorder::new();
    let mut stack = FixtureStack::new();

    for index in 0..count {
        let label = format!("f{index}");
        stack
            .setup_with(|| {
                let hook = recorder.hook(&label);
                async move { ((), hook) }
            })
            .await;
    }

    let report = stack.teardown_all().await;

    let expected: Vec<String> = (0..count).rev().map(|i| format!("f{i}")).collect();
    assert_eq!(report.ran, count);
    assert_eq!(recorder.seen(), expected);
}
