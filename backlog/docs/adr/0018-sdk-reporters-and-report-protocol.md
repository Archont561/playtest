# ADR 0018: SDK Reporters and the Report Protocol

## Status
Accepted (2026-11-11). Amends [ADR 0011](./0011-cross-language-cli-and-compatibility-architecture.md).

## Context

[ADR 0011](./0011-cross-language-cli-and-compatibility-architecture.md) lets `playtest test` run every detected native runner and aggregate the results. A runner's exit status and JUnit file do not say in which test a process crashed, which fixture phase failed, or whether the SDK in that process has the CLI's version.

One way to close the gap is a runner in every SDK, started as `python -m playtest` and its equivalents. That replaces pytest, `go test`, and the other native runners with nine new ones. [Spec 0008](../specs/0008-cross-language-cli-and-toolchain-detection.md) states that the CLI does not replace the native test frameworks, and [Spec 0005 §2](../specs/0005-polyglot-1-to-1-fixture-and-bdd-parity.md) maps every target to one.

## Decision

1. **SDKs expose a reporter, not a runner.** The CLI starts the native runner. The SDK hooks into that runner and reports what it observes.
2. **The reporter is switched on by the environment.** The CLI sets `PLAYTEST_REPORT_DIR` and `PLAYTEST_PROTOCOL` (names provisional). Without them the reporter does nothing, so a direct `pytest` or `go test` run is unchanged.
3. **Each process writes one NDJSON file** into the report directory, one record per line, flushed after every line.
4. **The records are** `hello` (protocol, SDK version, language, framework), `found`, `start`, `fixture` (name and phase), `end` (outcome, duration, message, reproduction hint), and `bye` (exit). `found` and `bye` are written only where the runner offers a hook for them. The record types live in `playtest-core` ([Spec 0014 §3](../specs/0014-report-protocol-and-runner-orchestration.md)).
5. **The CLI works in five phases**: plan, spawn, handshake, reconcile, report. It starts all selected targets at once, each in its own process group with its own report directory.
6. **The handshake checks versions.** A `hello` whose protocol or SDK version does not match the CLI ends the run with exit status 3.
7. **The CLI reconciles three sources**: the runner's exit status, the protocol records, and the runner's own report. A `start` without an `end` is a crash in that test. An exit status of 0 with no records and no native report is an environment failure, not a pass. A target can never pass on its exit status alone.
8. **Where the reporter cannot see every test, the native report fills in.** Python, C++, Java, R, and C reporters see all tests. TypeScript, Go, Rust, and C# reporters see the tests declared through Playtest, and the rest come from JUnit, `go test -json`, or TRX output.

## Alternatives Rejected

- **A runner in every SDK.** See Context.
- **Parsing the runner's standard output.** Formats differ by runner and version, and user output interleaves with them.
- **A Bun preload alone for TypeScript.** Bun's test hooks receive no test name and no result, so a preload cannot observe a plain Bun test. The SDK's `test` wrapper reports its own tests, and Bun's JUnit file covers the rest.

## Consequences

### Positive
- The native runner stays the source of truth, and running it directly keeps working.
- A crash is located at the test that was running.
- A target that produced no evidence cannot pass by exiting with status 0.
- SDK and CLI versions are compared on every run.

### Negative
- Four reporters see only part of the suite, and tests taken from the native report carry less detail.
- Nine reporters and one protocol version must be maintained.
- Reporters for pytest, Bun, Go, and GoogleTest were prototyped on 2026-11-11 outside this repository. Those for Java, R, C#, Rust, and C are designed from documentation and untested.

## Open Questions

1. The environment variable names are provisional (`task_060`).
2. What the CLI does when a native report exists but no process of the target wrote a `hello`. The prototype treated it as an environment failure. A workspace may also hold a project that does not use the SDK (`task_063`).
3. The C harness is not selected (`task_031`), so the C reporter has no host yet.
4. Whether the protocol is frozen with `v1.0.0`. Because the CLI and the SDKs share one version ([ADR 0015](./0015-cli-first-distribution-and-five-crate-workspace.md)), the protocol never has to work across versions, which argues against freezing it.
