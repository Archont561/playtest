# playtest

A next-generation, polyglot testing framework with dependency-injected fixtures, strict LIFO teardown, property-based testing, and fluent BDD scenarios across 9 language ecosystems.

## Polyglot Subprojects & Crates

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
| **Automation** | Python CLI | `scripts/` | `scripts/automation.py` |

## Quick Start

```bash
# Install pixi dependencies across all ecosystems
pixi install

# Setup git hooks
pixi run install-hooks

# Run tests across all 9 polyglot ecosystems
pixi run test-all

# Run universal LCOV code coverage aggregation
pixi run coverage-all

# Lint and check formatting repository-wide
pixi run lint-all
pixi run fmt-check
```

## Tooling & Verification

All quality gates run hermetically via Pixi:

| Command | Action |
|:--|:--|
| `pixi run fmt-check` | Check code formatting across all 9 language targets |
| `pixi run lint-all` | Lint entire monorepo (`clippy`, `ruff`, `biome`, `clang-format`, `dotnet format`, `golangci-lint`, `taplo`, `actionlint`) |
| `pixi run test-all` | Execute all test suites (`nextest`, `pytest`, `bun test`, `ctest`, `dotnet test`, `go test`, `Rscript`) |
| `pixi run coverage-all` | Aggregate polyglot coverage into unified `coverage/lcov.info` |
| `pixi run package-all` | Build release packages across all 9 ecosystems |
| `pixi run detect-release`| Dry-run conventional commits change & release scope detection |
| `pixi run audit` | Audit Rust dependencies for CVEs via `cargo-audit` |
| `pixi run sandbox-plan` | Verify offline sandbox packaging plan |
| `pixi run automation <args>` | Execute repository automation commands via Python |

## Development Container

A ready-to-use Dev Container configuration is available in `.devcontainer/`:
- Automatic Pixi toolchain bootstrap (`.devcontainer/setup.sh`)
- Pre-installed Rust, `pixi-sandbox`, `git`, and `gh`
- Pre-configured VS Code extensions (Rust Analyzer, Even Better TOML, Error Lens)

## Task Backlog

Task planning and tracking is managed in `backlog/`:
- Configuration: `backlog/config.toml`
- Tasks: `backlog/tasks.yaml`
