# Specification 0014: Report Protocol and Native-Runner Orchestration

## Overview

Specifies how the CLI runs each target's native test runner and how the SDK inside that runner reports back, as decided in [ADR 0018](../adr/0018-sdk-reporters-and-report-protocol.md). It refines `playtest test` in [Spec 0008](./0008-cross-language-cli-and-toolchain-detection.md) and the CLI boundary in [Spec 0005 §4](./0005-polyglot-1-to-1-fixture-and-bdd-parity.md). Target milestone: M5 (`v0.5.0`).

**This is a target contract. None of it is implemented yet.** Delivery is tracked by `task_060` to `task_063`.

Terms:

- **Reporter**: the part of an SDK that observes the native runner from inside the test process and writes protocol records.
- **Native report**: what the runner itself can emit, such as JUnit XML, `go test -json` output, or a TRX file.
- **Evidence**: the protocol records and the native report of one target.

## 1. Roles

1. The CLI starts the native runner. An SDK provides no runner of its own, such as `python -m playtest`.
2. The reporter observes. It never changes which tests run or how they end.
3. A direct run of the native runner, without the CLI, behaves as if no reporter existed.

## 2. Activation and Transport

1. The CLI sets two environment variables for each runner process: `PLAYTEST_REPORT_DIR`, a directory, and `PLAYTEST_PROTOCOL`, the protocol version it speaks. The names are provisional until `task_060`.
2. When either variable is unset, the reporter does nothing.
3. Each process writes one file into the report directory. Two processes never share a file.
4. The file is NDJSON: one JSON object per line. The reporter flushes after every line, so a crash loses at most the line being written.
5. The reporter writes only to its file, never to standard output or standard error.

## 3. Records

| Record | Written when | Required | Fields |
| :--- | :--- | :--- | :--- |
| `hello` | first, before any other record | yes | `protocol`, `sdk` (SDK version), `lang`, `framework` |
| `found` | a test is discovered | where the runner exposes collection | `id` |
| `start` | a test begins | for every test the reporter sees | `id` |
| `fixture` | a fixture is set up or torn down | for every fixture phase of those tests | `id`, `name`, `phase` |
| `end` | a test finishes | for every started test that finishes | `id`, `outcome`, `ms`, `message`, `repro` |
| `bye` | last, when the process ends normally | where the runner offers a hook at process end | `exit` |

- Every line also names its record and carries a timestamp. The prototypes used the keys `e` and `t`.
- `id` identifies one test within one target and is the same in every record about that test.
- `outcome` distinguishes at least pass, failure, and skip.
- `repro` carries the seed or replay command of a failed property test, as [Spec 0008 §7](./0008-cross-language-cli-and-toolchain-detection.md) requires.
- A missing `bye` is not evidence of a crash. The crash signal is a `start` without an `end` (§6).
- In the prototypes the pytest reporter wrote all six records. The Bun and Go reporters wrote no `bye`, and the Go and GoogleTest reporters wrote no `found`.
- The record types are defined in `playtest-core`. `task_060` fixes the remaining field details and records them here.

## 4. Per-Language Integration

| Language | The CLI runs | SDK hook | The reporter sees | Prototyped |
| :--- | :--- | :--- | :--- | :--- |
| Python | `python -m pytest` | pytest plugin registered by the wheel's entry point | every test | yes |
| C++ | CTest or GoogleTest binaries | header that attaches a GoogleTest listener before `main` | every test | yes |
| Java | `mvn test` or Gradle | JUnit `TestExecutionListener` registered through `META-INF/services` | every test | no |
| R | `Rscript -e 'playtest::run()'` | testthat reporter | every test | no |
| C | CTest or test binaries | Playtest's own harness, to be selected (`task_031`) | every test | no |
| TypeScript | `bun test --preload <reporter> --reporter=junit` | preload plus the SDK's `test` wrapper | tests declared through Playtest; the rest come from JUnit | yes |
| Go | `go test -json ./...` | SDK helpers | tests that use Playtest; the rest come from `-json` | yes |
| Rust | `cargo nextest run` with JUnit output | the `test!` macro family ([Spec 0002](./0002-declarative-and-attribute-macros.md)) | tests declared through Playtest; the rest come from JUnit | no |
| C# | `dotnet test --logger trx` | fixture wrapper | tests that use Playtest; the rest come from TRX | no |

"Prototyped" means tried outside this repository on 2026-11-11. The other rows are designed from the frameworks' documentation.

Notes:

- `playtest::run()` only calls testthat with the Playtest reporter. testthat remains the runner. The entry point does not exist yet — the package exports only `VERSION` and `install_cli` — so the R row's status values describe the `test_check()` call it will make. `test_check()` defaults to `stop_on_failure = TRUE`; `test_dir()` defaults to `FALSE` and would exit `0` on failures, so a bare `test_dir()` must not be used as the runner.
- Bun's test hooks receive no test name and no result, so a preload alone cannot observe a plain Bun test.
- nextest's machine-readable event stream is experimental, so the Rust row uses its JUnit output.
- This table decides the exact command. The default commands in [Spec 0008 §1](./0008-cross-language-cli-and-toolchain-detection.md) name the runner that is detected.

### 4.1 Runner Exit Statuses

The raw exit status of a runner is an input to reconciliation (§6), not the verdict. The nine runners disagree on what a non-zero status means, and several of them report "nothing ran" as success. This table gives, per runner, the status for a test failure, for a build failure, for a toolchain that is not installed, and for a selection that ran no tests.

| Language | Test failure | Build failure | Missing toolchain | No tests selected |
| :--- | :--- | :--- | :--- | :--- |
| Rust (nextest) | `100` | `101` | `127` (process not found) | `4` |
| Python (pytest) | `1` | `1` (collection error is `2`) | `127` | `5` |
| TypeScript (`bun test`) | `1` | `1` | `127` | `0` — **indistinguishable from success** |
| Go (`go test`) | `1` | `1` | `127` | `0` — reported as `succeeded` with no tests |
| C / C++ (CTest) | `8` | configure/build fails before CTest (`1`) | `127` | `0` |
| C# (`dotnet test`) | `1` | `1` | `127` | `0` unless `TreatNoTestsAsError` is set |
| Java (`mvn test`) | `1` | `1` | `127` | `0` |
| Java (Gradle) | `1` | `1` | `127` | `0` |
| R (`Rscript`) | `1` via `test_check()` | `1` | `127` | `0` — `test_dir()` defaults to `stop_on_failure = FALSE` |

Verified by execution on 2026-11-11 for nextest `0.9.151` (`0` / `100` / `101`), pytest (`0` / `1` / collection error `2` / usage `4` / no tests `5`), and `bun test` (`0` / `1` / `0` with no test files). The CTest, `dotnet test`, Maven, Gradle, Go, and R rows are taken from their documentation and are re-verified when each integration is built. `127` is produced by the shell, not the runner, and is what a missing toolchain looks like when the command cannot be executed at all.

Four findings drive the rules below:

1. **`1` is overloaded.** For pytest, Go, bun, CTest's configure step, `dotnet test`, Maven, Gradle, and R, `1` covers both "tests failed" and "the build or collection failed". A build failure must not be reported as a test failure: the first means the code is broken, the second means an assertion did not hold.
2. **`0` does not mean tests passed.** `bun test` exits `0` when the filter selects no test files, and `go test` exits `0` and prints `ok … [no test files]` for a package with none. Reading only the status would report an empty selection as a green run.
3. **Missing toolchain is not a runner status.** A toolchain that is absent means the command never runs, so the operating system reports `127` (Unix) or `%ERRORLEVEL%` 9009 (Windows). Any other status came from a process that did start.
4. **pytest reuses small integers.** `2` is an interrupted run and `4` is a command-line usage error, so neither may be read as a test failure even though `1` was. Only `1`, `5`, and `0` carry test-run meaning.

Rules:

- `100` from nextest is the only unambiguous test-failure status among the nine. Every other runner requires the evidence check below.
- A target whose status is `0` **and** whose reporter and native report contain no executed test is never a pass. It resolves through §6: environment failure (exit status `3`), because the target produced no evidence. An explicit selection that the CLI's own planner already knows matches no test target is the usage error of [Spec 0008 §6](./0008-cross-language-cli-and-toolchain-detection.md) (exit status `2`) and is decided before the runner starts. This is what makes Bun's and Go's `0` safe.
- A non-zero status other than the table's test-failure value is a build, configuration, or environment failure. It maps to exit status `3`, and the failure names the build rather than a test, unless the reporter recorded an `end` record for a test, in which case that test is reported as failing.
- A `127` (or Windows `9009`) from the process table, or any failure to spawn at all, is a missing toolchain and maps to exit status `3` with the remediation `playtest doctor` would give. This satisfies the fail-loud requirement of [Spec 0008 §6](./0008-cross-language-cli-and-toolchain-detection.md).
- When the status and the evidence disagree, §6 decides and the output shows both sources. The exit status alone never establishes a pass.

No new exit codes are introduced: every case above resolves to one of the four codes in [Spec 0008 §7](./0008-cross-language-cli-and-toolchain-detection.md).

## 5. Orchestration Phases

1. **Plan.** Detect targets from their manifests and map the requested tier and filter to each runner (Spec 0008 §§1, 6).
2. **Spawn.** Start all selected targets at once. Each target gets its own process group and its own report directory. Cancellation and `--fail-fast` end the whole group.
3. **Handshake.** Read each `hello` and check it (§7).
4. **Reconcile.** Combine the exit status, the protocol records, and the native report of each target (§6).
5. **Report.** Emit pretty, JSON, or JUnit output, merge LCOV, and exit with a status of Spec 0008 §7.

## 6. Reconciliation Rules

The per-runner meaning of a raw exit status is in §4.1. These rules decide what happens when that status and the collected evidence describe different outcomes.

| Observation | Result |
| :--- | :--- |
| A `start` with no matching `end` | The test crashed. It is reported as a failure of that test, with the runner's exit status. |
| Exit status 0, no protocol records, and no native report | Environment failure (exit status 3). The target did not pass, because a `0` alone does not prove tests ran (§4.1). |
| A test is in the native report but not in the records | The test is taken from the native report and marked with that source in JSON output. |
| No `hello` from any process of a target | The reporter did not load, so the SDK version was not checked. The prototype reported this as an environment failure even when a native report existed. Whether a workspace may contain a target that does not use the SDK is settled by `task_063`. |
| The sources disagree in another way | The target is reported as failed, and the output shows each source. `task_063` details the cases. |

## 7. Versioning and Skew

1. `protocol` is an integer. A `hello` whose `protocol` the CLI does not support ends the run with exit status 3.
2. `sdk` must equal the CLI's version ([ADR 0015](../adr/0015-cli-first-distribution-and-five-crate-workspace.md)). A difference ends the run with exit status 3. The message names both versions and the fix.
3. A new optional field, or a new record that a reader may ignore, keeps the protocol version. Any other change increments it.

## 8. Boundaries

- A reporter failure, such as an unwritable report directory, never changes a test's outcome. The CLI reports the missing evidence (§6).
- The protocol carries test events. Coverage stays with the LCOV contract of Spec 0008 §4.
- The protocol is internal to Playtest. Consumers read the JSON documents of Spec 0008 §7, not the protocol files.
- The CLI and the SDKs share one version, so the protocol does not have to work across versions. Whether it is frozen at `v1.0.0` is an open question of ADR 0018.

## 9. Verification

The implementing tasks provide at least:

- round-trip tests for every record type;
- shared protocol cases that every reporter passes: silence without the environment variables, `hello` first, `bye` last where the reporter writes it, one file per process, fixture phases attached to the right test;
- CLI tests for a crash between `start` and `end`, for a target that exits 0 without evidence, and for a protocol or SDK version mismatch.
