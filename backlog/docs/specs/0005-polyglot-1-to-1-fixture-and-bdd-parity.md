# Specification 0005: Polyglot 1:1 Fixture and BDD Parity

## Overview
Defines the cross-language architectural contract ensuring identical developer experience, fixture lifecycle semantics, and fluent BDD step transitions across all supported Playtest targets (Rust, Python, TypeScript, C++, Go, Java, R).

## Requirements

### 1. Fixture Lifecycle Contract
Every language implementation must guarantee:
1. **Lazy Resolution**: Fixtures are only evaluated if explicitly requested by the test signature or parameter list.
2. **Strict LIFO Teardown**: Teardown hooks must execute in exact reverse order of fixture creation.
3. **Panic & Failure Safety**: Teardown hooks must run unconditionally, even if the test body throws an exception, encounters an assertion failure, or panics.

### 2. Implementation Mappings

| Language | Test Framework | Fixture Injection | Teardown Mechanism | Property Testing |
| :--- | :--- | :--- | :--- | :--- |
| **Rust** | `nextest` | `test!("title", { f1, f2 } => async { ... })` | RAII Drop guard + Async teardown | `proptest` |
| **Python** | `pytest` | `@pytest.fixture` / `yield` context manager | Post-`yield` execution | `hypothesis` |
| **TypeScript** | `bun test` / Playwright | Destructured argument injection `{ f1, f2 }` | Fixture callback cleanup | `fast-check` |
| **C++** | GoogleTest | RAII Fixture class / template injection | Destructor / `TearDown()` | `RapidCheck` |
| **Go** | `testing` | Function parameters with context | `t.Cleanup(func() { ... })` | `rapid` |
| **Java** | JUnit 5 | `ParameterResolver` / `@ExtendWith` | `ExtensionContext.Store.CloseableResource` | `jqwik` |
| **R** | `testthat` | Environment scoping / helper functions | `on.exit(..., add = TRUE)` | `hedgehog` |

### 3. Fluent BDD Scenario Contract
Scenario pipelines across all languages must support the standard state machine:
```
Given(state_setup) -> When(action) -> Then(assertion) -> And(additional_assertion)
```
- No Gherkin `.feature` files required.
- Steps pass state forward to subsequent steps without global variables.
