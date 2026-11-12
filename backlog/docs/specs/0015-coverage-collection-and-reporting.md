# Specification 0015: Coverage Collection and Reporting

## Overview

Specifies `playtest coverage`, the orchestration command that runs each selected target's
native coverage tool and merges the resulting LCOV tracefiles into one report. It refines
the LCOV contract of [Spec 0008 §4](./0008-cross-language-cli-and-toolchain-detection.md),
which fixed the merge semantics but left the command that performs a run unspecified, and
follows the delegation pattern of [Spec 0011](./0011-delegated-mutation-testing.md): the
engine is always the ecosystem's own instrumentation, never a Playtest reimplementation.

Target milestone: M5 (`v0.5.0`), tracked by `task_068`.

**This is a target contract. None of it is implemented in the binary yet.** What exists
today is the build-automation equivalent in `automation/merge_lcov.py` plus the per-target
`coverage-*` Pixi tasks, whose path-normalization rules this specification adopts so the
shipped command and this repository's own CI agree.

## 1. Positioning

- Coverage answers what executed; it does not answer whether the suite would catch a
  defect. That is the mutation score of [Spec 0011](./0011-delegated-mutation-testing.md),
  and the two are reported separately because they answer different questions.
- Instrumentation stays native: `cargo llvm-cov`, `pytest --cov` with `pytest-cov`,
  `bun test --coverage`, `gcov`/`lcov` for C and C++, `dotnet --collect` with Coverlet,
  `go test -coverprofile` with the repository's converter, JaCoCo for Java, and `covr` for
  R. Each target's documentation declares the authoritative mapping, exactly as runner
  mappings do ([Spec 0008 §1](./0008-cross-language-cli-and-toolchain-detection.md)).
- Coverage is a quality signal. Like mutation scores, coverage numbers never count as
  conformance or parity evidence ([Spec 0005](./0005-polyglot-1-to-1-fixture-and-bdd-parity.md)).
- `playtest test` already merges LCOV as part of its report phase
  ([Spec 0014 §5](./0014-report-protocol-and-runner-orchestration.md)); `playtest coverage`
  is the standalone form of the same merge, for callers that want only coverage and no
  test-report document.

## 2. Relationship to `playtest test`

The two commands share their plan and spawn phases and differ in what they report:

| | `playtest test` | `playtest coverage` |
| :--- | :--- | :--- |
| Runs tests | yes | yes |
| Emits a test report | yes | no, unless `--format json` is requested |
| Merges LCOV | yes (Spec 0014 §5) | yes, and it is the primary artifact |
| Fails on test failure | yes | yes, unless `--ignore-test-failures` |
| Coverage threshold | no | `--min-lines` |

Neither command replaces the other. A caller that needs both test outcomes and coverage in
one process uses `playtest test`. A caller that needs only the coverage number, for
example a badge job or a coverage gate that should not re-parse test results, uses
`playtest coverage`.

## 3. CLI Grammar

```text
playtest coverage [OPTIONS]

Options:
  --lang <TARGET>    Select one or more targets (as in Spec 0008 §3)
  --tier <TIER>      Select a declared test tier (as in Spec 0008 §6)
  --filter <PAT>     Restrict the run via the native runner's filter mapping
  --output <PATH>    Merged LCOV destination (default: coverage/lcov.info)
  --format <FMT>     Output: pretty (default), json
  --min-lines <PCT>  Fail when merged line coverage drops below PCT
  --min-branches <PCT>  Fail when merged branch coverage drops below PCT
  --ignore-test-failures  Report coverage even when tests fail
  --skip-unavailable Report explicitly selected but unavailable targets as skipped
  --fail-fast        Stop after the first failing target
```

`--format junit` is rejected with exit `2`: JUnit XML describes test cases, and a
coverage document cannot be expressed in it. Coverage is reported through `pretty` and
`json` only.

Selection follows [Spec 0008 §6](./0008-cross-language-cli-and-toolchain-detection.md).
`playtest coverage` is by definition an explicit request, so a selected target whose
coverage tool is missing exits `3` with remediation rather than skipping green, and a
selection that matches no target exits `2`.

## 4. Per-Target Collection Contract

Each target produces an LCOV tracefile at the location its mapping declares. The CLI reads
those files; it does not parse any proprietary intermediate format.

| Target | Instrumentation the CLI runs | Tracefile written |
| :--- | :--- | :--- |
| Rust | `cargo llvm-cov nextest --lcov` | `coverage/lcov-rs.info` |
| Python | `pytest --cov --cov-report=lcov` | `python/coverage/lcov-py.info` |
| TypeScript | `bun test --coverage --coverage-reporter=lcov` | `typescript/coverage/lcov.info` |
| C++ | `gcov` plus `lcov` capture | `cpp/coverage/lcov-cpp.info` |
| C | `gcov` plus `lcov` capture | `c/coverage/lcov.info` |
| C# | `dotnet test --collect` with an LCOV coverlet format | `csharp/coverage/lcov.info` |
| Go | `go test -coverprofile` plus the LCOV converter | `go/coverage/lcov.info` |
| Java | JaCoCo report plus the LCOV converter | `java/coverage/lcov.info` |
| R | `covr` with an LCOV writer | `r/coverage/lcov.info` |

The Rust tracefile lives at the repository root because the Cargo workspace is rooted
there; the other eight live beside their subproject.

### Current deviation: eight sources, not nine

The build-automation precursor merges eight of these nine. `automation/merge_lcov.py`
lists no R source, and the `coverage-r` Pixi task runs the R test suite without
instrumentation, so it emits no tracefile and contributes nothing to the merge. The
existing `coverage-all` task therefore produces an eight-language report while presenting
itself as polyglot.

This specification fixes R as a required ninth source rather than silently documenting the
gap: a merged report that omits a target understates what was measured and inflates the
apparent result. Closing it is an acceptance criterion of `task_068`, and the R writer that
`covr` exposes is confirmed by that task before the row above is treated as normative.

## 5. Path Normalization

Merged `SF:` records are repository-relative and use forward slashes, so one report is
valid regardless of the host platform:

- An `SF:` value under the repository root loses that prefix.
- A target-relative value gains the target's directory prefix (`python/`, `go/`, …).
- Backslashes are converted to forward slashes.
- A Windows drive-letter or UNC prefix is dropped rather than emitted into the report.

A source file that cannot be resolved to the repository still appears in the report, with
the unresolved path preserved, because silently dropping a record makes the total coverage
look better than it is.

## 6. Merged Report Record

`--format json` emits one document under the machine contract of
[Spec 0008 §7](./0008-cross-language-cli-and-toolchain-detection.md) — stdout data only,
diagnostics on stderr, no prompts or color, `schema_version` present — carrying at least:

- `targets` — one record per target with `lang`, `engine`, `engine_version`,
  `tracefile`, `lines_found`, `lines_hit`, `branches_found`, `branches_hit`, and a
  `line_rate` / `branch_rate` pair computed from them.
- `merged` — the same counters summed across targets, with `output` naming the written
  LCOV path.
- `excluded` — targets excluded by the selection, with the reason, so a narrower run is
  distinguishable from a target that reported nothing.
- `thresholds` — each configured minimum and whether it passed, so a failure explains
  itself without the caller re-deriving the arithmetic.

A target that produced no tracefile is recorded with an explicit `no_coverage` status
rather than being omitted, because an absent target silently reduces the merged
denominator.

## 7. Exit Status

`playtest coverage` reuses the four codes of [Spec 0008 §7](./0008-cross-language-cli-and-toolchain-detection.md)
and adds no new ones:

- `0` — every selected target produced coverage and every configured threshold passed.
- `1` — a test failed (unless `--ignore-test-failures`) or a threshold was not met.
- `2` — usage or selection error: unknown target or tier, no matching target,
  `--format junit`, or a threshold outside 0–100.
- `3` — environment failure: a selected target's coverage tool is missing or unqualified,
  or no tracefile could be read at all.

A threshold failure is `1`, not `3`, because a low score is a property of the code under
test, not of the host.

## 8. Doctor Integration

- Coverage tools are **optional tools**: `playtest doctor` reports their presence and
  version when a workspace declares coverage configuration, and never fails a run merely
  because an untouched target's coverage tool is absent.
- An explicitly selected target's missing tool is reported as it would be for `test`
  ([Spec 0008 §6](./0008-cross-language-cli-and-toolchain-detection.md)), with the same
  remediation text.
- Minimum versions follow the compatibility matrix of
  [Spec 0008 §2](./0008-cross-language-cli-and-toolchain-detection.md); the Python row's
  `pytest-cov` and the Rust row's `cargo-llvm-cov` are recorded there.

## 9. Boundaries and Non-Goals

- No coverage engine, parser for a proprietary format, or instrumentation pass is
  implemented inside Playtest. The CLI reads LCOV and runs the native tool.
- No coverage-guided fuzzing and no differential-coverage analysis; fuzzing engines are an
  explicit non-goal ([`caveats.md` §5](../caveats.md)).
- Coverage never serves as conformance evidence, parity evidence, or a substitute for the
  mutation score; the three are separate report sections and separate exit reasons.
- Uploading the merged report to an external service is out of scope. The command writes a
  file; a consumer's CI uploads it.
