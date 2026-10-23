# Caveats & Non-Goals

This document records deliberate omissions, constraints, and non-goals in `playtest`.

## 1. Zero Browser Automation (Explicit Non-Goal)

Unlike `bun-test-utils`, `playtest` explicitly **does not include headless browser automation** (Chromium/Firefox/WebKit via CDP or WebDriver):
- **Rationale**: Rust applications are overwhelmingly backends, CLIs, microservices, and system software. Embedding browser drivers balloons compilation time, pulls hundreds of megabytes of system binaries, and introduces flakiness.
- **Alternative**: In-memory web server testing via `tower::ServiceExt::oneshot` and ephemeral lightweight HTTP mocks (`wiremock`).
- **Reference**: [ADR 0004](adr/0004-drop-browser-support-focus-on-backend.md).

## 2. No `.feature` Gherkin File Parser

Classic Cucumber/Gherkin requires parallel `.feature` plain text files and regex-matched step definitions. `playtest` deliberately uses a **code-first BDD pipeline**:
- `test_scenario!` uses native Rust method chaining (`.given().when().then()`).
- All state passing between steps is strongly typed at compile time.
- Direct IDE refactoring, type checking, and autocomplete.
- **Reference**: [ADR 0003](adr/0003-code-first-bdd-without-feature-files.md).

## 3. Multithreading & Environment Mutation Concurrency

In Rust, `cargo test` and `cargo-nextest` execute test cases in parallel across OS threads. Modifying global process state (such as `std::env::set_var`) causes data races:
- Tests using the `env` fixture or mutating global state must use `#[serial]` from `serial_test`.
- Thread-safe overrides should be favored over global environment mutation.

## 4. Procedural Macro Crate Boundary

Rust enforces that `proc-macro = true` crates cannot export normal runtime structs, traits, or constants:
- The workspace strictly maintains two primary crates: `playtest` (facade) and `playtest-macros` (proc-macros).
- Users depend only on `playtest`.
