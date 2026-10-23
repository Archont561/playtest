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
  --filter <PAT>   Filter tests using the native runner's supported filter mapping
  --format <FMT>   Output: pretty (default), json, junit
  --fail-fast      Stop after the first failing target
```

Unsupported filter semantics must be reported rather than silently ignored. Reports identify each target's command, exit status, test counts when available, and diagnostics.

## 4. Decentralized LCOV Coverage Contract

Each target's native runner or a target-specific converter emits an LCOV file at its documented location. The shared CLI/automation layer collects these files, normalizes `SF:` paths relative to the repository root, and merges them into the unified `coverage/lcov.info` report. The orchestrator should not parse every language's proprietary coverage format itself.

## 5. Conformance Boundary

CLI conformance is separate from framework feature parity in [Spec 0005](./0005-polyglot-1-to-1-fixture-and-bdd-parity.md). At minimum, CLI tests must verify detection/override behavior for all nine targets, target selection, failure/skip reporting, fail-fast behavior, and normalized coverage paths. A target without an implemented framework feature can still be detected, but the CLI must not label that target fully conformant.
