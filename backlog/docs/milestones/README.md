# Playtest Milestones Roadmap

This directory tracks the evolutionary milestones of the **Playtest** polyglot testing ecosystem. Each milestone is strictly bound to a semantic version, encompasses designated target packages, and associates discrete delivery tasks.

## Release Cadence & Semantic Version Bindings

| Milestone | Target SemVer | Scope | Focus Area | Status |
| :--- | :--- | :--- | :--- | :--- |
| [**M1: Foundation & CLI-First Distribution**](./M1-v0.1.0-foundation.md) | `v0.1.0` | Polyglot (all 9 language targets) & shared CLI | Done: monorepo scaffolding, package reservations, two-phase CI/CD, VERSION exports. Open: five-crate workspace, `playtest` binary with `version`/`init`/`sync`, distribution channels, SDK mirrors | **In Progress** (reopened 2026-11-11) |
| [**M2: Core Fixture Engine & Macros**](./M2-v0.2.0-core-fixtures-and-macros.md) | `v0.2.0` | Rust, Python, TypeScript | AsyncFixture trait, LIFO teardown, declarative `test!` macro, pytest bridge | **Ready** |
| [**M3: Fluent BDD & PBT Integration**](./M3-v0.3.0-fluent-bdd-and-pbt.md) | `v0.3.0` | Rust, Python, TypeScript | Method chaining `.given().when().then()`, `#[prop]`, Hypothesis / proptest | **Backlog** |
| [**M4: Isolation, Snapshots & VCR Cassettes**](./M4-v0.4.0-isolation-and-vcr-cassettes.md) | `v0.4.0` | Rust, Python, TypeScript | `tmpdir` sandboxes, `snap` auto-redactions, `cassette` HTTP VCR | **Backlog** |
| [**M5: Polyglot Core Conformance & Cross-Lang CLI**](./M5-v0.5.0-polyglot-conformance.md) | `v0.5.0` | All 9 language targets & shared CLI | Core fixture/BDD/PBT parity, conformance suite, workspace auto-detection, `playtest doctor`, `playtest test` with SDK reporters, delegated `playtest coverage` and `playtest mutation` | **Backlog** |
| [**M6: Universal Stable Production Release**](./M6-v1.0.0-universal-stable-release.md) | `v1.0.0` | Polyglot (all 9 language targets) | Stable APIs, frozen fixture extension API (Spec 0010), documented core parity, performance benchmarks, coordinated `v1.0.0` | **Backlog** |
| [**M7: Ecosystem Integration Fixtures**](./M7-v1.1.0-ecosystem-integration-fixtures.md) | `v1.1.0` | Rust, Python & TypeScript | First-party fixture packages on Spec 0010: `playtest-clap`/`tui`/`tauri`, `playtest-qt`/`tk`, `playtest-browser` | **Backlog** |

M1 was reopened on 2026-11-11: [ADR 0015](../adr/0015-cli-first-distribution-and-five-crate-workspace.md) moves the CLI into the first release, and `v0.1.0` had not been tagged. M2 work can proceed in parallel, but `v0.2.0` is released after `v0.1.0`.

The files `M1-v0.1.0-fixture-engine.md`, `M2-v0.2.0-macro-attributes.md`, `M3-v0.3.0-fluent-bdd.md`, `M4-v0.4.0-standard-isolation-fixtures.md`, and `M5-v1.0.0-production-release.md` belong to an earlier Rust-only roadmap. They are superseded by the files linked above and kept for history.

---

## Milestone Release Lifecycle & Automation

This section describes the workflows as they are implemented. [ADR 0015](../adr/0015-cli-first-distribution-and-five-crate-workspace.md) gives every release one version across the CLI and all SDKs, which conflicts with package-scoped tags; `task_059` decides their future ([Spec 0007](../specs/0007-autorelease-change-detection-and-milestone-dispatch.md)).

Milestone releases in Playtest are driven by git tag pushes and Conventional Commits:
1. **Pushing a Milestone Tag**:
   Pushing a tag formatted as `milestone-v<SEMVER>` or `milestone/v<SEMVER>` (e.g. `git push origin milestone-v0.2.0`) triggers `.github/workflows/auto-release.yml`.
2. **Auto-Release Execution**:
   * Evaluates the milestone version target and inspects git history using `python -m automation.release detect`.
   * Enforces quality gates (`pixi run fmt-check`, `pixi run lint-all`, `pixi run test-all`).
   * Verifies local packaging dry-runs (`pixi run package-all`).
   * Cuts and pushes the formal release tag: `v0.2.0` for a general release, or a package-scoped tag such as `python-v0.2.0` or `go/v0.2.0` when only one target changed.
3. **Two-Phase Publication**:
   * The release tag triggers `.github/workflows/release.yml`.
   * **Phase 1**: Builds distribution packages into isolated GitHub Actions artifacts.
   * **Phase 2**: Downstream jobs publish to registries where automated publishing is supported and configured: crates.io, PyPI, npm, Go proxy indexing, Conan remotes, NuGet, and Maven Central. R packages receive a CRAN-style check; CRAN submission is manually confirmed, while R-universe tracks GitHub releases once its registry and app are configured. GitHub Releases retain the built artifacts as the fallback distribution channel.
   * From `v0.1.0` the release also carries the `playtest` binary: per-platform archives on the GitHub Release, and the registry channels of [Spec 0012](../specs/0012-workspace-crates-and-cli-distribution.md). This is not implemented yet (`task_054` to `task_058`).
