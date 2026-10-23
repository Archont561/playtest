# playtest

A polyglot testing framework targeting consistent core testing behavior across nine language ecosystems. The target contract covers dependency-injected fixtures, failure-safe LIFO teardown, property-based testing, and fluent BDD scenarios, with language-idiomatic APIs and implementations.

> **Parity is a project goal, not a claim that every capability is implemented in every target today.** See [Spec 0005](backlog/docs/specs/0005-polyglot-1-to-1-fixture-and-bdd-parity.md) for the shared behavioral contract and [the milestone roadmap](backlog/docs/milestones/README.md) for delivery status.

## Language Targets

| Language | Ecosystem | Directory | Package / Artifact |
| :--- | :--- | :--- | :--- |
| **Rust** | Cargo / crates.io | `crates/playtest`, `crates/playtest-macros` | `playtest`, `playtest-macros` |
| **Python** | PyPI / pytest | `python/` | `playtest-py` (wheel & sdist) |
| **TypeScript** | npm / bun | `typescript/` | `@archont561/playtest` |
| **C++** | CMake / Conan | `cpp/` | `playtest-cpp` |
| **C** | CMake / C11 | `c/` | `playtest-c` |
| **C#** | .NET / NuGet | `csharp/` | `Playtest` (.nupkg) |
| **Go** | Go Modules / `pkg.go.dev` | `go/` | `github.com/Archont561/playtest/go` |
| **Java** | Maven Central | `java/` | `io.github.archont561:playtest` |
| **R** | CRAN / R-universe | `r/` | `playtest` |

## Core Feature Parity

Every supported language target is expected to provide the same observable core guarantees, using the idioms of its own ecosystem:

- Fixture injection and documented scope/resolution behavior.
- Strict reverse-creation-order (LIFO) teardown, including after assertion failures, exceptions, or panics.
- A code-first BDD pipeline that passes state explicitly through Given, When, Then, and And steps.
- Property testing with fresh fixture lifecycles per generated sample, shrinking, and reproducible failure seeds or cases.
- Equivalent skip/conditional-skip and table-driven/parameterized test controls.
- Shared isolation and deterministic-test helpers defined by the specifications, with clear mappings per language.

The cross-language `playtest` CLI is a shared orchestration tool, not a requirement to duplicate the CLI in every language. It must detect and run every supported target and normalize results and coverage. Language-specific test libraries may differ; their behavior must satisfy the common contract and pass the conformance suite.

## Quick Start

```bash
# Install Pixi dependencies
pixi install

# Setup git hooks
pixi run install-hooks

# Run tests across all language targets
pixi run test-all

# Run universal LCOV code coverage aggregation
pixi run coverage-all

# Lint and check formatting repository-wide
pixi run lint-all
pixi run fmt-check
```

## Tooling & Verification

All quality gates run via Pixi. A passing aggregate command verifies the configured targets; consult the parity specification and milestone status before assuming a particular feature has shipped in every language.

| Command | Action |
|:--|:--|
| `pixi run fmt-check` | Check formatting across the configured language targets |
| `pixi run lint-all` | Run repository and per-language lint checks |
| `pixi run test-all` | Run the configured test suites across all language targets |
| `pixi run coverage-all` | Aggregate polyglot coverage into unified `coverage/lcov.info` |
| `pixi run package-all` | Build release packages across the configured ecosystems |
| `pixi run detect-release` | Dry-run conventional-commit change and release-scope detection |
| `pixi run audit` | Audit Rust dependencies via `cargo-audit` |
| `pixi run sandbox-plan` | Verify the offline sandbox packaging plan |
| `pixi run automation <args>` | Execute repository automation via Python |

## Development Container

A ready-to-use Dev Container configuration is available in `.devcontainer/`:
- Automatic Pixi toolchain bootstrap (`.devcontainer/setup.sh`)
- Pre-installed Rust, `pixi-sandbox`, `git`, and `gh`
- Pre-configured VS Code extensions (Rust Analyzer, Even Better TOML, Error Lens)

## Task Backlog

Task planning and tracking is managed in `backlog/`:
- Configuration: `backlog/config.toml`
- Tasks: `backlog/tasks.yaml`
