# Spec 0001: Fixture Engine, Lifecycles, and LIFO Teardown

## 1. Overview
The fixture engine provides dependency-injected test context modeled after Playwright. Tests request only the fixtures they need, and the engine resolves and unwinds them.

## 2. Core Traits

```rust
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait AsyncFixture: Sized {
    type Output;
    fn setup<'a>() -> BoxFuture<'a, (Self::Output, Box<dyn FnOnce() -> BoxFuture<'static, ()> + Send>)>;
}

pub struct FixtureHandle<T> {
    pub val: T,
    teardown: Option<Box<dyn FnOnce() -> BoxFuture<'static, ()> + Send>>,
}
```

## 3. Invariants
1. **LIFO Teardown**: If fixtures `A`, `B`, and `C` are resolved in that order, teardown MUST execute in order `C`, `B`, `A`.
2. **Panic Safety**: Teardown hooks MUST execute even if the test body panics (`catch_unwind`).
3. **No Leaked State**: Every fixture handle clears its resources when the test terminates.
