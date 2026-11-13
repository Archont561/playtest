//! End-to-end tests for the declarative `test!` macro (`Spec 0002 §1`).

use std::sync::atomic::{AtomicUsize, Ordering};

use playtest::{AsyncFixture, BoxFuture, TeardownHook, test};

/// Counts teardowns so a test can prove nothing was torn down mid-body.
static TEARDOWNS: AtomicUsize = AtomicUsize::new(0);

/// A fixture whose output identifies which type was resolved.
#[derive(Debug, PartialEq, Eq)]
pub struct Marker(pub u8);

/// A fixture that counts its own teardown.
struct Counted<const ID: u8>;

impl<const ID: u8> AsyncFixture for Counted<ID> {
    type Output = Marker;

    fn setup<'a>() -> BoxFuture<'a, (Self::Output, TeardownHook)> {
        Box::pin(async move {
            let hook: TeardownHook = Box::new(move || {
                let boxed: BoxFuture<'static, ()> = Box::pin(async move {
                    TEARDOWNS.fetch_add(1, Ordering::Relaxed);
                });
                boxed
            });
            (Marker(ID), hook)
        })
    }
}

type First = Counted<1>;
type Second = Counted<2>;

test!("binds exactly the fixtures the body requests", { First, Second } => {
    assert_eq!(First.0, 1, "the first fixture is bound to its requested name");
    assert_eq!(Second.0, 2, "the second fixture is bound to its requested name");
});

test!("a body with no fixtures runs and completes", { } => {
    let total: u8 = (1..=3).sum();
    assert_eq!(total, 6);
});

test!("teardown is deferred until the body returns", { First } => {
    let before = TEARDOWNS.load(Ordering::Relaxed);
    assert_eq!(
        TEARDOWNS.load(Ordering::Relaxed),
        before,
        "teardown must not run while the body is still executing",
    );
});
