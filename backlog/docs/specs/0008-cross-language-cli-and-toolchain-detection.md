# Specification 0008: Cross-Language CLI & Toolchain Detection

## Overview

Specifies the workspace discovery protocol, manifest signatures, toolchain compatibility checks, and diagnostic/reporting behavior for the shared `playtest` CLI under **Milestone M5 (`v0.5.0`)**. The CLI is one shared Rust tool that orchestrates all nine language targets; it does not replace their native test frameworks or implement their framework APIs.

## 1. Workspace Project Auto-Detection

The CLI inspects the active repository or requested directory and identifies the following targets:

| Target | Manifest signatures | Default test framework / command |
| :--- | :--- | :--- |
| **Rust** | `Cargo.toml` | `cargo nextest run --all-features` (fallback: `cargo test`) |
| **Python** | `pyproject.toml`, `pytest.ini`, `setup.cfg` | `pytest` |
| **TypeScript** | `package.json` with a supported Bun/Node lockfile | `bun test` / configured runner |
| **C++** | CMake/C++ sources or documented C++ manifest | CTest / configured C++ runner |
| **C** | CMake/C sources or documented C manifest | configured C test executable / CTest |
| **C#** | `*.csproj`, `*.sln`, or `*.slnx` | `dotnet test` |
| **Go** | `go.mod` | `go test ./...` |
| **Java** | `pom.xml`, `build.gradle`, `build.gradle.kts` | Maven or Gradle test task |
| **R** | `DESCRIPTION` | `Rscript` with the configured testthat suite |

C and C++ can share CMake files, so detection must inspect declared project languages/targets or require an explicit override; it must not guess solely from the presence of `CMakeLists.txt`. When several supported runners exist, selection and override behavior must be deterministic and visible in `playtest list`/diagnostics.

## 2. Toolchain Compatibility (`playtest doctor`)

The diagnostic engine probes the configured tools and compares them to the compatibility matrix. Initial minimums are:

| Target | Minimum toolchain |
| :--- | :--- |
| Rust | Rust 1.80+ and Cargo; Nextest when configured as the runner |
| Python | Python 3.10+ and pytest; pytest-cov for coverage |
| TypeScript | Node 18+ or Bun 1.0+; the project selects its runner |
| C++ | GCC 11+ or Clang 14+ with C++20 support |
| C | C11-capable compiler supported by the target manifest |
| C# | .NET SDK 10+ (the current projects target `net10.0`) |
| Go | Go 1.21+ |
| Java | OpenJDK 11+ |
| R | R 4.1+ |

The exact matrix must remain synchronized with CI, Pixi manifests, and published target support. Diagnostics must distinguish required from optional tools, identify missing/incompatible versions, and give actionable remediation without claiming that the framework's features are fully conformant merely because a runtime is present.

## 3. CLI Command Grammar

```text
playtest [COMMAND] [OPTIONS]

Commands:
  test             Execute tests across detected subprojects
  list             List detected projects, runtimes, and test targets
  doctor           Check toolchain compatibility and environment health
  version          Print the CLI and target package versions

Options:
  --lang <TARGET>  Select one or more targets: rust, python, ts, cpp, c, csharp, go, java, r
  --tier <TIER>    Select a declared test tier (e.g. unit, smoke)
  --filter <PAT>   Filter tests using the native runner's supported filter mapping
  --format <FMT>   Output: pretty (default), json, junit
  --fail-fast      Stop after the first failing target
  --skip-unavailable  Report explicitly selected but unavailable targets as skipped (see §6)
```

Unsupported filter semantics must be reported rather than silently ignored. Reports identify each target's command, exit status, test counts when available, and diagnostics.

## 4. Decentralized LCOV Coverage Contract

Each target's native runner or a target-specific converter emits an LCOV file at its documented location. The shared CLI/automation layer collects these files, normalizes `SF:` paths relative to the repository root, and merges them into the unified `coverage/lcov.info` report. The orchestrator should not parse every language's proprietary coverage format itself.

## 5. Conformance Boundary

CLI conformance is separate from framework feature parity in [Spec 0005](./0005-polyglot-1-to-1-fixture-and-bdd-parity.md). At minimum, CLI tests must verify detection/override behavior for all nine targets, target selection, failure/skip reporting, fail-fast behavior, and normalized coverage paths. CLI tests must also cover the selection semantics in §6, the machine consumption contract in §7, and probe folding in §8. A target without an implemented framework feature can still be detected, but the CLI must not label that target fully conformant.

## 6. Explicit Selection and Tier Semantics

`--lang`, `--tier`, and `--filter` are *explicit requests*: once the caller narrows the run, the availability of the selected targets is their concern and must not disappear into a green skip.

- Declared tiers are, at minimum, `unit` and `smoke`. Each target maps a tier to its native mechanism (for example a pytest marker expression or a native name filter); a target without a native tier mechanism uses a documented convention. Every target's effective tier mapping is visible in `playtest list`.
- A tier that no detected target defines, or a selection that matches no test target, is a usage error reported with exit status 2 — never a successful empty run.
- When an explicitly selected target cannot run (missing toolchain, unqualified host, missing required runtime), `playtest test` exits with the environment status (exit status 3) and emits the remediation `playtest doctor` would give for it. Reporting the target as skipped and exiting 0 is non-conforming.
- `--skip-unavailable` restores skip reporting for explicitly selected unavailable targets; the skipped result and its reason stay visible in pretty, JSON, and JUnit outputs.
- Targets excluded by the selection are reported as excluded and do not affect the exit status.
- Framework-side parity: when a tier is requested through a target's native runner directly (for example `pytest -m smoke`), Playtest-provided runner integrations apply the same rule — an explicitly requested capability whose host prerequisites are missing fails with an actionable reason instead of skipping green. Capabilities that were never requested keep the ordinary conditional-skip behavior of [Spec 0005 §1.4](./0005-polyglot-1-to-1-fixture-and-bdd-parity.md).

## 7. Machine Consumption Contract

The CLI is a programmable interface first; `pretty` is a rendering of the same data that `json` and `junit` carry.

Exit status:

| Code | Meaning |
| :--- | :--- |
| 0 | All selected targets ran and passed; policy skips and exclusions allowed |
| 1 | At least one test failure |
| 2 | Usage or selection error (unknown target or tier, no matches, invalid flag) |
| 3 | Environment failure: an explicitly selected target is unavailable, or `doctor` found a required toolchain or probe failure |

JSON rules:

- Every `--format json` document carries a `schema_version`. Additive changes keep the current version; breaking changes bump it and are recorded in this specification in the same change.
- In `json` (and `junit`) mode, stdout carries only the report document; progress, warnings, and diagnostics go to stderr. Machine modes never prompt and never emit ANSI color. `NO_COLOR` is honored in `pretty` mode.
- Failures identify the failing target, the executed command, and the failing test ids. A property-test failure includes the seed or replay command needed to reproduce the case.
- `playtest version` and command help point to the shipped agent guidance (consumer skill and the specification anchors above) so an automated consumer can locate the contract from the tool itself.

## 8. Extensible Diagnostic Probes

`playtest doctor` cannot know consumer-specific runtimes, but consumer qualification workflows — for example verifying an application install prefix and its Python bindings before running a headless smoke tier — need the same required/optional/remediation semantics as built-in checks.

- Consumers register probes through workspace configuration discovered by the CLI; the registration format is versioned alongside the JSON schema.
- Each probe yields a record with at least `id`, `description`, `observed`, `ok`, `required`, and `remediation`.
- Probe results fold into pretty and JSON doctor output next to built-in checks and obey the same exit-status rules.
- A passing probe reports environment readiness only; it must never assert framework feature parity (§5 boundary).
