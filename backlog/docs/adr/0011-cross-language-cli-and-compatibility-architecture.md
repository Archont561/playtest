# ADR 0011: Cross-Language CLI & Toolchain Compatibility Architecture

## Status
Accepted

## Context
As Playtest expands to 7 programming languages, running tests requires developers and CI pipelines to remember individual, disparate commands (`cargo nextest`, `pytest`, `bun test`, `ctest`, `go test`, `mvn test`, `Rscript`). Furthermore, developer workstations frequently experience toolchain drift (e.g. outdated compiler versions, missing test runner plugins, incompatible runtime flags). A single unified CLI is needed to discover project manifests, evaluate local toolchain health, and orchestrate polyglot test runs under a single command.

## Decision
Under **Milestone M5 (`v0.5.0`)**, we introduce the **Unified Polyglot CLI** (`playtest`):
1. **Manifest Auto-Discovery**:
   - Recursively inspects the workspace for language manifests (`Cargo.toml`, `pyproject.toml`, `package.json`, `CMakeLists.txt`, `go.mod`, `pom.xml`, `DESCRIPTION`).
   - Automatically maps detected directories to their canonical test runners.
2. **Health & Compatibility Engine (`playtest doctor`)**:
   - Probes host binaries for runtime versions against the official compatibility matrix (Rust 1.80+, Python 3.10+, Node 18+, Go 1.21+, Java 11+, C++20, R 4.1+).
   - Provides actionable remediation suggestions when required toolchains or plugins are missing.
3. **Unified Execution & Reporting**:
   - `playtest test` executes all detected test runners sequentially or concurrently.
   - Aggregates multi-language test results into unified summaries, JSON outputs, and standard JUnit XML files for CI consumption.

## Consequences
### Positive
- Single command (`playtest test`) runs polyglot suites without memorizing disparate flags.
- `playtest doctor` eliminates "works on my machine" debugging by instantly identifying version discrepancies.
- Provides consistent JUnit XML outputs across polyglot builds for CI reporting.

### Negative
- Requires maintaining detection logic and version parsers across 7 different toolchains.
