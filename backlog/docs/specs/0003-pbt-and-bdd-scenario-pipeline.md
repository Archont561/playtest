# Spec 0003: Fluent Scenario State Machine & Property-Based BDD Pipeline

## 1. Syntax

```rust
test_scenario! {
    "order checkout", { db, snap }
        .given("an order with items", |_fix| Order::new().with_item("Book", 20))
        .when("checkout runs", |_fix, mut order| { order.checkout(); order })
        .then("status is completed", |_fix, order| assert!(order.is_completed()))
        .and("matches snapshot", |fix, order| fix.1.match_json(&order))
}
```

## 2. Type-Driven State Transitions

```rust
pub struct ScenarioBuilder<'a, Fix, State> {
    pub title: &'static str,
    pub fix: &'a mut Fix,
    pub state: State,
}
```
- `.given<S2>(FnOnce(&mut Fix) -> S2)` transitions from `()` to `S2`.
- `.when<S3>(FnOnce(&mut Fix, S2) -> S3)` transitions from `S2` to `S3`.
- `.then(FnOnce(&mut Fix, &S3))` asserts without state mutation.

## 3. Property Combination
`#[prop(...)]` decorates `test_scenario!` directly, feeding arbitrary generators into step parameters and executing full shrinking on assertion failure.
