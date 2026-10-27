# Playtest Milestones Roadmap

This directory tracks the evolutionary milestones of the **Playtest** polyglot testing ecosystem. Each milestone is strictly bound to a semantic version, encompasses designated target packages, and associates discrete delivery tasks.

## Release Cadence & Semantic Version Bindings

| Milestone | Target SemVer | Scope | Focus Area | Status |
| :--- | :--- | :--- | :--- | :--- |
| [**M1: Foundation & Package Reservation**](./M1-v0.1.0-foundation.md) | `v0.1.0` | Polyglot (all 9 language targets) | Monorepo scaffolding, package reservations, two-phase CI/CD, VERSION exports | **Completed** |
| [**M2: Core Fixture Engine & Macros**](./M2-v0.2.0-core-fixtures-and-macros.md) | `v0.2.0` | Rust, Python, TypeScript | AsyncFixture trait, LIFO teardown, declarative `test!` macro, pytest bridge | **Ready** |
| [**M3: Fluent BDD & PBT Integration**](./M3-v0.3.0-fluent-bdd-and-pbt.md) | `v0.3.0` | Rust, Python, TypeScript | Method chaining `.given().when().then()`, `#[prop]`, Hypothesis / proptest | **Backlog** |
| [**M4: Isolation, Snapshots & VCR Cassettes**](./M4-v0.4.0-isolation-and-vcr-cassettes.md) | `v0.4.0` | Rust, Python, TypeScript | `tmpdir` sandboxes, `snap` auto-redactions, `cassette` HTTP VCR | **Backlog** |
| [**M5: Polyglot Core Conformance & Cross-Lang CLI**](./M5-v0.5.0-polyglot-conformance.md) | `v0.5.0` | All 9 language targets & shared CLI | Core fixture/BDD/PBT parity, conformance suite, workspace auto-detection, `playtest doctor` | **Backlog** |
| [**M6: Universal Stable Production Release**](./M6-v1.0.0-universal-stable-release.md) | `v1.0.0` | Polyglot (all 9 language targets) | Stable APIs, frozen fixture extension API (Spec 0010), documented core parity, performance benchmarks, coordinated `v1.0.0` | **Backlog** |
| [**M7: Ecosystem Integration Fixtures**](./M7-v1.1.0-ecosystem-integration-fixtures.md) | `v1.1.0` | Rust, Python & TypeScript | First-party fixture packages on Spec 0010: `playtest-clap`/`tui`/`tauri`, `playtest-qt`/`tk`, `playtest-browser` | **Backlog** |

---

## Milestone Release Lifecycle & Automation

Milestone releases in Playtest are driven by git tag pushes and Conventional Commits:
1. **Pushing a Milestone Tag**:
   Pushing a tag formatted as `milestone-v<SEMVER>` or `milestone/v<SEMVER>` (e.g. `git push origin milestone-v0.2.0`) triggers `.github/workflows/auto-release.yml`.
2. **Auto-Release Execution**:
   * Evaluates the milestone version target and inspects git history using `python scripts/detect_release.py`.
   * Enforces quality gates (`pixi run fmt-check`, `pixi run lint-all`, `pixi run test-all`).
   * Verifies local packaging dry-runs (`pixi run package-all`).
   * Cuts and pushes the formal release tag (`v0.2.0` or `<scope>-v0.2.0`).
3. **Two-Phase Publication**:
   * The release tag triggers `.github/workflows/release.yml`.
   * **Phase 1**: Builds distribution packages into isolated GitHub Actions artifacts.
   * **Phase 2**: Downstream jobs publish to registries where automated publishing is supported and configured: crates.io, PyPI, npm, Go proxy indexing, Conan remotes, NuGet, and Maven Central. R packages receive a CRAN-style check; CRAN submission is manually confirmed, while R-universe tracks GitHub releases once its registry and app are configured. GitHub Releases retain the built artifacts as the fallback distribution channel.
