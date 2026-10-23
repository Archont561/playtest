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
- `test!` expands into an `async` function executed under `#[tokio::test]`.
- `#[prop(...)]` parses the macro invocation, wraps it in a `proptest!` engine, and recreates/tears down fixtures for each generated permutation.
