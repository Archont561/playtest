# ADR 0002: Async Fixture Trait and Strict LIFO Teardown

## Context
Playwright and `bun-test-utils` rely on generator/coroutine pausing (`await use(val)`), where code before `use` is setup and code after is teardown. In Rust, destructors via the `Drop` trait are synchronous only and cannot `await` async network/pool closures.

## Decision
1. Define an `AsyncFixture` trait returning the fixture output and an async teardown future closure:
   `fn setup<'a>() -> BoxFuture<'a, (Self::Output, Box<dyn FnOnce() -> BoxFuture<'static, ()> + Send>)>;`
2. Wrap values in `FixtureHandle<T>` implementing `Deref` and `DerefMut`.
3. Enforce strict LIFO teardown: fixtures tear down in exact reverse order of resolution.
4. Use `std::panic::AssertUnwindSafe` to ensure teardowns execute even when test assertions fail.

## Consequences
- Full support for async database connections, server shutdowns, and file flushes.
- No leaky state between tests.
