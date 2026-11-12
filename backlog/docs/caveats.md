# Caveats & Non-Goals

This document records deliberate omissions, constraints, and non-goals in `playtest`.

## 1. Zero Browser Automation in Core (Explicit Non-Goal)

Unlike `bun-test-utils`, `playtest` explicitly **does not include headless browser automation** (Chromium/Firefox/WebKit via CDP or WebDriver):
- **Rationale**: Rust applications are overwhelmingly backends, CLIs, microservices, and system software. Embedding browser drivers balloons compilation time, pulls hundreds of megabytes of system binaries, and introduces flakiness.
- **Alternative**: In-memory web server testing via `tower::ServiceExt::oneshot` and ephemeral lightweight HTTP mocks (`wiremock`).
- **Reference**: [ADR 0004](adr/0004-drop-browser-support-focus-on-backend.md).
- **Scope update**: [ADR 0013](adr/0013-scoped-browser-drop-and-ecosystem-integration-fixtures.md) keeps this true *for core* while permitting an opt-in, separately shipped TypeScript provider package with browser fixtures after `v1.0.0` ([Spec 0009](specs/0009-ecosystem-integration-fixtures.md)).

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
- Procedural macros live in `playtest-macros`, and `playtest` re-exports them.
- Users depend only on `playtest`.
- The workspace has five crates in total ([ADR 0015](adr/0015-cli-first-distribution-and-five-crate-workspace.md), which supersedes ADR 0001): `playtest`, `playtest-macros`, `playtest-core`, `playtest-assets`, and `playtest-cli`. `playtest` never depends on the assets or the CLI.

## 5. Testing Techniques: In and Out of Scope

Where each class of testing technique belongs in Playtest:

- **In core** (deterministic, pre-deployment, assertion-shaped): fixture/dependency-injection testing, code-first BDD, property-based testing with shrinking, parameterized/table-driven cases, snapshot/golden testing (`snap`), record/replay HTTP (`cassette`), hermetic isolation (`tmpdir`, `network_guard`), deterministic time and randomness (`clock`, `seed`), skip/selection controls, and cross-language conformance (Specs 0001–0005).
- **Delegated to native tools, orchestrated by the CLI**: coverage (native instrumentation and LCOV aggregation, [Spec 0008 §4](specs/0008-cross-language-cli-and-toolchain-detection.md), [Spec 0015](specs/0015-coverage-collection-and-reporting.md)) and mutation testing (native engines such as `cargo-mutants`/`mutmut`/Stryker, [Spec 0011](specs/0011-delegated-mutation-testing.md)) — Playtest aggregates normalized results and never implements the engines.
- **Ecosystem (post-v1 provider packages)**: GUI/TUI/browser qualification and smoke tiers, visual image goldens, and future contract-testing or narrow fault-injection tooling as provider candidates ([Spec 0009](specs/0009-ecosystem-integration-fixtures.md), [ADR 0014](adr/0014-v1-stability-boundary-and-fixture-extension-model.md)).
- **Expressible without dedicated APIs**: metamorphic and differential testing — property recipes on top of Spec 0003.
- **Out of scope**:
  - **A/B testing** — production traffic statistics and product experimentation; not pre-deployment correctness, and structurally incompatible with deterministic assertion-based testing.
  - **Canary / progressive deployments** — release engineering and observability, owned by deployment tooling. (Distinct from the validation "canary" used for API freezes, `task_048`.)
  - **Load / stress / performance testing of user applications** — a statistical, long-running tool class (k6/JMeter/Locust); Playtest benchmarks only its own overhead (`task_028`).
  - **Security testing** (SAST/DAST/scanners) — compliance tooling, not assertion testing.
  - **Chaos engineering platforms** — production-system discipline; only narrow fault injection into `cassette`/`network_guard` semantics may appear as a future ecosystem addition.
  - **Exploratory / manual test-management systems** — documentation and process, not executable contracts.
  - **Coverage-guided fuzzing engines** (libFuzzer/AFL class) — a possible future per-target integration, never part of the shared cross-language contract; in-framework fuzzing is covered by property-based testing.
  - **Race / interleaving model checking** (loom-class) — a language-runtime primitive; ecosystem candidate at best, out of contract.

## 6. CLI-First Distribution: Limits and Costs

The `playtest` binary is the version authority, and SDKs reach a repository through the binary or beside it ([ADR 0015](adr/0015-cli-first-distribution-and-five-crate-workspace.md) to [ADR 0018](adr/0018-sdk-reporters-and-report-protocol.md)). The model has deliberate limits:

- **One version, no mixing.** A CLI and an SDK of different versions are an error, not a supported combination. There is no compatibility matrix between versions.
- **`init` and `sync` never edit consumer manifests.** Wiring a project to its SDK is one explicit command of the ecosystem's own tool. It is repeated when the mirror's path changes, which for Python and TypeScript is every upgrade. npm and Bun do not report a forgotten wiring command while their cache holds the old SDK ([Spec 0013 §5](specs/0013-workspace-configuration-and-sdk-mirrors.md)).
- **Mirrors are committed files.** A consumer repository carries a wheel or a tarball per mirrored language and reviews it as a diff on every upgrade.
- **Nothing downloads at install time.** The Go and R wrappers download the binary on first use and need network access once. No package runs a lifecycle script.
- **No SDK ships a test runner.** The CLI starts each native runner; the SDK only reports from inside it. Running the native runner directly stays supported and unchanged.
- **No native binary channel for Java, C#, C, or C++ in `v0.1.0`.** Those repositories take the binary from GitHub Releases, conda, or another channel.
- **Staged delivery.** Milestone M1 (`task_050` to `task_059`) delivers the five-crate workspace, `version`/`init`/`sync`, deterministic mirrors, and multi-channel binary packaging; runner orchestration (`list`, `doctor`, `test`, `mutation`) and SDK JSONL reporters remain scheduled for Milestone M5 (`task_060` to `task_064`).
