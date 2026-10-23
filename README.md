# playtest

A polyglot testing framework targeting consistent core testing behavior across nine language ecosystems. The target contract covers dependency-injected fixtures, failure-safe LIFO teardown, property-based testing, and fluent BDD scenarios, with language-idiomatic APIs and implementations.

> **Parity is a project goal, not a claim that every capability is implemented in every target today.** See [Spec 0005](backlog/docs/specs/0005-polyglot-1-to-1-fixture-and-bdd-parity.md) for the shared behavioral contract and [the milestone roadmap](backlog/docs/milestones/README.md) for delivery status.

## Language Targets

| Language | Ecosystem | Directory | Package / Artifact |
| :--- | :--- | :--- | :--- |
| **Rust** | Cargo / crates.io | `crates/playtest`, `crates/playtest-macros` | `playtest`, `playtest-macros` |
| **Python** | PyPI / pytest | `python/` | `playtest-py` (wheel & sdist) |
| **TypeScript** | npm / bun | `typescript/` | `@archont561/playtest` |
| **C++** | CMake / Conan 2 | `cpp/` | `playtest-cpp` (configured Conan remote; GitHub Release fallback) |
| **C** | CMake / Conan 2 | `c/` | `playtest-c` (configured Conan remote; GitHub Release fallback) |
| **C#** | .NET / NuGet | `csharp/` | `Playtest` (.nupkg; NuGet publish when configured) |
| **Go** | Go Modules / `pkg.go.dev` | `go/` | `github.com/Archont561/playtest/go` (tag-indexed) |
| **Java** | Maven Central | `java/` | `io.github.archont561:playtest` (publish when configured) |
| **R** | CRAN / R-universe | `r/` | `playtest` (CRAN submission is manually confirmed; R-universe tracks GitHub releases) |

Release tags build the selected packages and attach the artifacts to GitHub Releases. Conan, NuGet, and Maven Central publishing is enabled by registry credentials in the documented GitHub environments; without those credentials, registry publishing is skipped. CRAN submission is a separately confirmed workflow dispatch; R-universe builds from the tagged Git source after its registry and GitHub app are configured. Ensure `r/DESCRIPTION` in the tagged source already has the tag's version—CI-only version stamping does not change the Git tag. See [AGENTS.md](AGENTS.md#registry-configuration) for setup details.

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
