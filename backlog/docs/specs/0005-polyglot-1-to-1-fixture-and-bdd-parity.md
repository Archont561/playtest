# Specification 0005: Polyglot Core Feature Parity

## Overview

This specification defines the shared **behavioral contract** for all nine Playtest language targets: Rust, Python, TypeScript, C++, C, C#, Go, Java, and R. It is a target for parity, not a claim that every target has implemented every feature. Public APIs and syntax may be idiomatic to each language; observable semantics and conformance requirements must agree.

Language-specific implementation and delivery status belongs in the milestone roadmap and backlog. Do not infer feature support from a package existing or a toolchain being installed.

## 1. Core feature contract

Every target implementation must provide the following equivalent user-facing capabilities:

### 1.1 Fixtures and lifecycle

1. **Explicit resolution:** tests request the fixtures they use; unused fixtures are not constructed merely because they are registered.
2. **Documented scopes:** per-test and broader fixture scopes have consistent lifetime and isolation semantics. A broader-scoped fixture must not accidentally leak mutable state between tests.
3. **Deterministic teardown:** cleanup runs in strict reverse order of successful fixture creation (LIFO).
4. **Failure safety:** teardown continues after ordinary assertion/test failures, exceptions, or panics. One failing cleanup must not prevent remaining cleanups from being attempted; failures are reported without hiding the original test failure.
5. **Fresh property samples:** each generated property-test sample gets a fresh lifecycle for its per-test fixtures.

C has no built-in exception mechanism; its adapter must define recoverable assertion/failure behavior that still permits registered teardown to run. Unrecoverable process termination (for example, an OS kill or memory fault) is outside this guarantee.

### 1.2 BDD scenarios

- Support a code-first Given → When → Then → And pipeline without requiring Gherkin files.
- Pass state explicitly from one step to the next; do not require hidden global state.
- Stop running subsequent scenario steps after a failed step and still perform fixture teardown.

### 1.3 Property-based testing

- Generate multiple inputs using the language's native property-testing ecosystem.
- Recreate per-test fixtures for each generated sample.
- Shrink a failing sample to a smaller counterexample.
- Report enough seed/case information to reproduce a failure. The documented replay mechanism must work in CI and locally.

### 1.4 Test controls

- Provide unconditional and condition-based skipping with an explicit skipped result.
- Provide table-driven or generated parameterized cases and identify the failing case/input in reports.
- Preserve the underlying runner's useful diagnostics while exposing a consistent Playtest-level outcome.

### 1.5 Isolation and deterministic helpers

The portable behavior of the shared helpers in Spec 0004 is also part of the parity target: temporary-directory isolation and traversal protection, deterministic clock/seed behavior, snapshots, HTTP record/replay, and an outbound-network guard. Implementations can use native libraries, but must meet the documented behavior and have conformance coverage. A target must not silently omit one of these helpers while claiming full core parity.

## 2. Idiomatic implementation mapping

The table names intended native integrations, not an API-syntax mandate. “To select” means the language-specific implementation task must make and document that choice before claiming support.

| Language | Native test integration | Fixture/cleanup mapping | Property-testing integration |
| :--- | :--- | :--- | :--- |
| **Rust** | `nextest` / Rust test harness | fixture handles and panic-safe guards | `proptest` |
| **Python** | `pytest` | fixture injection and yield cleanup | `hypothesis` |
| **TypeScript** | Bun test (and supported Playwright integration) | injected fixture values and callback cleanup | `fast-check` |
| **C++** | GoogleTest | RAII fixture and teardown hooks | RapidCheck |
| **C** | C test harness (select and document) | explicit fixture context and registered cleanup stack | Select and document a C-compatible generator/property engine |
| **C#** | xUnit | fixture/context injection and disposal | Select and document a .NET property-testing engine |
| **Go** | `testing` | `t.Cleanup`-based cleanup stack | `rapid` |
| **Java** | JUnit 5 | extension/context-managed resources | `jqwik` |
| **R** | `testthat` | scoped helpers and `on.exit` cleanup | `hedgehog` |

A selected library does not define parity by itself. Each adapter must pass the same behavioral contract tests.

## 3. Cross-language conformance suite

The conformance suite must exercise every target, not only the languages that have implementation tasks today. Use shared, language-neutral cases where possible and test observable behavior rather than implementation details. At minimum, verify:

- unused fixture is not initialized; requested fixtures initialize in a known order;
- teardown is reverse-creation-order after success, assertion failure, and cleanup failure;
- fixture scopes do not leak mutable state between isolated tests;
- BDD state passes through each step and execution stops after failure;
- a property sample receives a fresh fixture lifecycle, and a failed case is replayable;
- skip and parameterized-case outcomes identify the correct reason/input;
- temporary resources are cleaned, snapshot masking is stable, VCR replay is deterministic, and disallowed network access is blocked;
- each target's native runner can be invoked and its outcomes can be normalized by the shared CLI.

The suite must report results per language and per capability. Full parity is achieved only when every required capability passes for all nine targets; partial implementations must be reported as partial, not as universal support.

## 4. Shared CLI boundary

The cross-language CLI is one shared tool, not nine independent CLI implementations. It must detect every supported target, run its configured native test command, support target selection, and normalize process status, test summaries, and LCOV coverage. CLI orchestration does not substitute for the framework features in §§1–3.

## 5. Compatibility and evolution

- Keep public syntax idiomatic; keep behavior and failure guarantees equivalent.
- Document any native-runner limitation and provide a consistent Playtest-level alternative where practical.
- Add or change a core capability by updating this contract, its conformance cases, and all target mappings together.
- Do not mark a language fully conformant until every required capability has evidence from the conformance suite.
