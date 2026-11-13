//! Dependency-injected fixtures with strict LIFO teardown (`Spec 0001`).
//!
//! A fixture is a value a test asks for by name; the engine constructs it, hands over the
//! output, and records how to tear it down. Two types carry that contract:
//!
//! - [`FixtureHandle`] owns one fixture output and its teardown hook. It derefs to the
//!   output and, if dropped while still armed, tears down on a best-effort basis.
//! - [`FixtureStack`] owns the teardown hooks of every fixture a test resolved and runs
//!   them in reverse resolution order. [`run_with_teardown`] adds panic safety: the hooks
//!   run whether the body returns or unwinds.
//!
//! The ordering guarantee is the point. If a test resolves `a`, then `b`, then `c`,
//! teardown runs `c`, `b`, `a` — a dependency can never observe a resource that depended on
//! it being released first.

use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::panic::{AssertUnwindSafe, resume_unwind};
use std::pin::Pin;

use futures::FutureExt;

/// A boxed, boxed-once future that may borrow for `'a` (`Spec 0001 §2`).
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// The async cleanup a fixture performs when its test ends (`Spec 0001 §2`).
///
/// The hook is `'static` and `Send`, so it can outlive the borrow that created it and be
/// spawned onto a runtime when a handle is dropped instead of torn down explicitly.
pub type TeardownHook = Box<dyn FnOnce() -> BoxFuture<'static, ()> + Send>;

/// A resource a test can ask the engine to construct (`Spec 0001 §2`).
///
/// `setup` returns the fixture's output together with the hook that releases it. It takes
/// no argument because the fixture decides what it needs; anything the hook requires must
/// be owned by the hook or shared behind a handle such as [`std::sync::Arc`].
pub trait AsyncFixture: Sized {
    /// What the test body receives.
    type Output;

    /// Construct the fixture, returning its output and its teardown hook.
    fn setup<'a>() -> BoxFuture<'a, (Self::Output, TeardownHook)>;
}

/// One resolved fixture: its output, and the hook that releases it (`Spec 0001 §2`).
///
/// The handle derefs to the output, so a test body uses the value directly. Teardown runs
/// exactly once: through [`FixtureHandle::teardown`], through the owning [`FixtureStack`],
/// or — as a last resort when the handle is dropped while still armed — by spawning the
/// hook onto the current runtime.
pub struct FixtureHandle<T> {
    /// The fixture's output.
    pub val: T,
    teardown: Option<TeardownHook>,
}

impl<T> FixtureHandle<T> {
    /// Wrap an output and its teardown hook.
    #[must_use]
    pub fn new(val: T, teardown: TeardownHook) -> Self {
        Self {
            val,
            teardown: Some(teardown),
        }
    }

    /// Whether the fixture still needs to be released.
    #[must_use]
    pub fn is_armed(&self) -> bool {
        self.teardown.is_some()
    }

    /// Release the fixture now, returning whether a hook actually ran.
    ///
    /// After this call the handle is disarmed, so dropping it does nothing.
    pub async fn teardown(&mut self) -> bool {
        let Some(hook) = self.teardown.take() else {
            return false;
        };
        hook().await;
        true
    }
}

impl<T> Deref for FixtureHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

impl<T> DerefMut for FixtureHandle<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.val
    }
}

impl<T> Drop for FixtureHandle<T> {
    /// Best-effort release for a handle that was never torn down (`Spec 0001 §3.3`).
    ///
    /// Dropping cannot await, so an armed hook is spawned onto the ambient runtime. Without
    /// a runtime there is nowhere to run it, and the handle leaves the resource alone
    /// rather than blocking; [`FixtureStack`] is the ordering-aware path and never relies
    /// on this fallback. A `Drop` spawn may run out of order with respect to other
    /// fixtures, which is precisely why it is a fallback and not the normal path.
    fn drop(&mut self) {
        let Some(hook) = self.teardown.take() else {
            return;
        };
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(hook());
        }
    }
}

/// Everything one test resolved, released in reverse resolution order (`Spec 0001 §3.1`).
///
/// A stack holds the teardown hooks only; the values live wherever the test bound them.
/// That keeps the ordering guarantee simple: hooks run from last pushed to first.
#[derive(Default)]
pub struct FixtureStack {
    hooks: Vec<TeardownHook>,
}

impl FixtureStack {
    /// An empty stack.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many fixtures are still awaiting teardown.
    #[must_use]
    pub fn len(&self) -> usize {
        self.hooks.len()
    }

    /// Whether every fixture has been released.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty()
    }

    /// Register a teardown hook. Hooks run in reverse registration order.
    pub fn push(&mut self, hook: TeardownHook) {
        self.hooks.push(hook);
    }

    /// Resolve a fixture from its [`AsyncFixture`] implementation and register its teardown.
    pub async fn setup<F: AsyncFixture>(&mut self) -> F::Output {
        self.setup_with(F::setup).await
    }

    /// Resolve a fixture from a caller-supplied constructor and register its teardown.
    ///
    /// [`FixtureStack::setup`] is the [`AsyncFixture`]-driven form; this one carries
    /// fixture state that `setup` cannot reach, such as a per-test recorder.
    pub async fn setup_with<C, Fut, T>(&mut self, construct: C) -> T
    where
        C: FnOnce() -> Fut,
        Fut: Future<Output = (T, TeardownHook)>,
    {
        let (value, hook) = construct().await;
        self.push(hook);
        value
    }

    /// Run every registered hook in reverse registration order.
    ///
    /// A hook that panics does not stop the remaining hooks, and its message is recorded in
    /// the report rather than replacing the test's own outcome
    /// (`Spec 0005 §1.1`: cleanup must never swallow the original failure).
    pub async fn teardown_all(&mut self) -> TeardownReport {
        let mut report = TeardownReport::default();
        while let Some(hook) = self.hooks.pop() {
            report.ran += 1;
            let outcome = AssertUnwindSafe(hook()).catch_unwind().await;
            if let Err(payload) = outcome {
                report.failures.push(panic_message(payload));
            }
        }
        report
    }
}

/// What happened while releasing a test's fixtures.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TeardownReport {
    /// How many hooks ran.
    pub ran: usize,
    /// One message per hook that panicked, in the order the hooks ran.
    pub failures: Vec<String>,
}

impl TeardownReport {
    /// Whether every hook completed without panicking.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Run `body`, release every fixture, then surface the body's own outcome (`Spec 0001 §3.2`).
///
/// The body's panic is caught, not suppressed: teardown runs, and the panic is resumed
/// afterwards so the test still fails. Teardown failures are returned alongside the
/// result so a caller can report them without hiding why the test failed.
pub async fn run_with_teardown<Fut, T>(stack: &mut FixtureStack, body: Fut) -> (T, TeardownReport)
where
    Fut: Future<Output = T>,
{
    let outcome = AssertUnwindSafe(body).catch_unwind().await;
    let report = stack.teardown_all().await;
    match outcome {
        Ok(value) => (value, report),
        Err(payload) => resume_unwind(payload),
    }
}

/// Best-effort text for a caught panic payload.
fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    if let Some(text) = payload.downcast_ref::<&str>() {
        return (*text).to_owned();
    }
    if let Some(text) = payload.downcast_ref::<String>() {
        return text.clone();
    }
    "fixture teardown panicked with a non-string payload".to_owned()
}
