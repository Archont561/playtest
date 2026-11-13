# Spec 0002: Declarative and Attribute Macros

## 1. Syntax

### Declarative `test!` Macro
```rust
test!("user registration", { db, tmpdir } => {
    let user = db.create_user("Alice").await;
    assert!(user.is_valid());
});
```

### Attribute Decorators on `test!`
```rust
#[skip]
test!("disabled test", { tmpdir } => { ... });

#[skip_if(cfg!(target_os = "windows"))]
test!("unix only", { tmpdir } => { ... });

#[prop(age in 18..120u32, name in "[a-z]+")]
test!("validates arbitrary profiles", { db } => |age, name| {
    assert!(db.validate(age, &name).await);
});
```

## 2. Expansion Contract
- `test!` expands into a `#[test]` function that builds its own current-thread runtime and drives the body to completion. It does not expand into `#[tokio::test]`: a macro that required the caller's runtime attribute could not be invoked as a single declaration, and a proc-macro crate cannot re-export `tokio` at a path that attribute expansion can resolve.
- The fixture list in `{ ... }` names each fixture twice: the identifier is both the type to resolve through `AsyncFixture` and the local the body reads. `{ db, tmpdir }` therefore resolves `db::setup()` and binds the output to a local named `db`. A name that is not a type in scope is a compile error, which is the intended feedback.
- Fixtures are resolved in declaration order and released in the reverse order after the body returns, as [Spec 0001 §3.1](./0001-fixture-engine-and-lifecycle-scopes.md) requires. Teardown also runs when the body panics.
- The generated function name is derived from the test title, so several declarations can share one module.
- `test_skip!` and `skip_if!` expand into a `#[test]` that records the skip and prints one `SKIPPED <title>: <reason>` line. The library returns the record rather than printing it, because a test framework's reporter, not its library, owns output.
- `#[prop(...)]` parses the macro invocation, wraps it in a `proptest!` engine, and recreates/tears down fixtures for each generated permutation.
