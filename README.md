# playtest

A polyglot testing framework targeting consistent core testing behavior across nine language ecosystems. The target contract covers dependency-injected fixtures, failure-safe LIFO teardown, property-based testing, and fluent BDD scenarios, with language-idiomatic APIs and implementations.

> **Parity is a project goal, not a claim that every capability is implemented in every target today.** See [Spec 0005](backlog/docs/specs/0005-polyglot-1-to-1-fixture-and-bdd-parity.md) for the shared behavioral contract and [the milestone roadmap](backlog/docs/milestones/README.md) for delivery status.

## Language Targets

| Language | Ecosystem | Directory | Package / Artifact |
| :--- | :--- | :--- | :--- |
| **Rust** | Cargo / crates.io | `crates/` | `playtest`, `playtest-macros`, `playtest-core`, `playtest-assets`, `playtest-cli` |
| **Python** | PyPI / pytest | `python/` | `playtest-py` (wheel & sdist) |
| **TypeScript** | npm / Bun | `typescript/` | `@archont561/playtest` |
| **C++** | CMake / Conan 2 | `cpp/` | `playtest-cpp` (configured Conan remote; GitHub Release fallback) |
| **C** | CMake / Conan 2 | `c/` | `playtest-c` (configured Conan remote; GitHub Release fallback) |
| **C#** | .NET / NuGet | `csharp/` | `Playtest` (.nupkg; NuGet publish when configured) |
| **Go** | Go Modules / `pkg.go.dev` | `go/` | `github.com/Archont561/playtest/go` (indexed from `go/vX.Y.Z` tags) |
| **Java** | Maven Central | `java/` | `io.github.archont561:playtest` (publish when configured) |
| **R** | CRAN / R-universe | `r/` | `playtest` (CRAN submission is manually confirmed; R-universe tracks GitHub releases) |

Release tags build the selected packages and attach the artifacts to GitHub Releases. Conan, NuGet, and Maven Central publishing is enabled by registry credentials in the documented GitHub environments; without those credentials, registry publishing is skipped. CRAN submission is a separately confirmed workflow dispatch; R-universe builds from the tagged Git source after its registry and GitHub app are configured. Ensure `r/DESCRIPTION` in the tagged source already has the tag's version—CI-only version stamping does not change the Git tag. See [AGENTS.md](AGENTS.md#registry-configuration) for setup details.

## Distribution Model (Planned)

**Not implemented yet.** No version has been released, and the registries hold only placeholder versions that reserve the package names. The table above shows the manifests as they are today.

From `v0.1.0`, the Rust CLI `playtest` is the version authority ([ADR 0015](backlog/docs/adr/0015-cli-first-distribution-and-five-crate-workspace.md)):

- **One version per release.** The CLI and every SDK share it. A CLI and an SDK of different versions are an error, not a supported combination.
- **Five crates.** `playtest`, `playtest-macros`, `playtest-core`, `playtest-assets`, and `playtest-cli`. Test code still declares only `playtest`.
- **One binary, many channels.** The binary embeds every SDK and reaches a repository through GitHub Releases, crates.io, PyPI, npm, conda, or a Go or R download wrapper ([ADR 0016](backlog/docs/adr/0016-cli-binary-distribution-channels.md)).
- **Pinned version and committed SDK mirrors.** `playtest init` pins the version in `playtest.toml`. `playtest sync` writes each SDK into `.playtest/`, and every project is wired to its mirror with its own package manager ([ADR 0017](backlog/docs/adr/0017-committed-sdk-mirrors-and-explicit-wiring.md)).
- **Native runners stay.** `playtest test` starts pytest, Bun, `go test`, and the other native runners, and each SDK reports from inside its runner ([ADR 0018](backlog/docs/adr/0018-sdk-reporters-and-report-protocol.md)).

[Spec 0012](backlog/docs/specs/0012-workspace-crates-and-cli-distribution.md) is the target contract. [Milestone M1](backlog/docs/milestones/M1-v0.1.0-foundation.md) tracks delivery of the binary, its channels, `init`, and `sync`; [Milestone M5](backlog/docs/milestones/M5-v0.5.0-polyglot-conformance.md) adds `list`, `doctor`, `test`, and `mutation`.

## Core Feature Parity

Every supported language target is expected to provide the same observable core guarantees, using the idioms of its own ecosystem:

- Fixture injection and documented scope/resolution behavior.
- Strict reverse-creation-order (LIFO) teardown, including after assertion failures, exceptions, or panics.
- A code-first BDD pipeline that passes state explicitly through Given, When, Then, and And steps.
- Property testing with fresh fixture lifecycles per generated sample, shrinking, and reproducible failure seeds or cases.
- Equivalent skip/conditional-skip and table-driven/parameterized test controls.
- Shared isolation and deterministic-test helpers defined by the specifications, with clear mappings per language.

The cross-language `playtest` CLI is a shared orchestration tool, not a requirement to duplicate the CLI in every language. It must detect and run every supported target and normalize results and coverage. Language-specific test libraries may differ; their behavior must satisfy the common contract and pass the conformance suite. The CLI does not exist yet; see [Distribution Model (Planned)](#distribution-model-planned).

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

All quality gates run through Pixi and the `doit` task graph in [`automation/dodo.py`](automation/dodo.py). A passing aggregate command verifies the configured targets; consult the parity specification and milestone status before assuming a particular feature has shipped in every language.

| Command | Action |
|:--|:--|
| `pixi run do fast` | Run repository structure, Clean Code, workflow, TOML, and automation gates |
| `pixi run do clean_code` | Run vendored Alint, Semgrep, ast-grep, Vale, and `rude-clean-code` checks |
| `pixi run do test_automation` | Run unit tests and strict `mypy` type checking for `automation/` |
| `pixi run fmt-check` | Check formatting across the configured language targets |
| `pixi run lint-all` | Run repository and per-language lint checks |
| `pixi run test-all` | Run the configured test suites across all language targets |
| `pixi run coverage-all` | Aggregate polyglot coverage into unified `coverage/lcov.info` |
| `pixi run package-all` | Dry-run release packaging across the configured ecosystems |
| `pixi run audit-local` | Run offline Rust policy checks (`cargo-deny` + advisory expiry check) |
| `pixi run sandbox-check` | Verify `pixi-sandbox` generated workflows and `scripts/restore.sh` |

## Documentation

- [Documentation Index](docs/README.md)
- [Development Setup](docs/guides/development-setup.md)
- [Clean Code Rule Register](docs/concepts/rule-register.md)
- [Contributing Guide](CONTRIBUTING.md)
- [Agent Directives](AGENTS.md)
- [Architecture Decision Records](backlog/docs/adr/README.md) & [Specifications](backlog/docs/specs/README.md)

## Development Container

A ready-to-use Dev Container configuration is available in `.devcontainer/`:
- Automatic Pixi toolchain bootstrap (`.devcontainer/setup.sh`)
- Pre-installed Rust, `pixi-sandbox`, `git`, and `gh`
- Pre-configured VS Code extensions (Rust Analyzer, Even Better TOML, Error Lens)

## Task Backlog

Task planning and tracking is managed in `backlog/`:
- Configuration: `backlog/config.toml`
- Tasks: `backlog/tasks.yaml`
