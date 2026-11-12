# AGENTS.md — Playtest Autonomous Agent Directives

Welcome to **Playtest**. This document defines the mandatory operating guidelines, architectural constraints, toolchain protocols, and release workflows for all AI agents contributing to this repository. These instructions are authoritative; the focused skills below add procedures without overriding them.

## Agent skills

- `.agents/skills/arena-agent/SKILL.md` — Arena-oriented operating guide and repository boundaries.
- `.agents/skills/session/SKILL.md` — session orientation, task selection, verification, and hand-off; use the templates in the same directory where helpful.
- `.agents/skills/audit/SKILL.md` — evidence-based, read-only code-quality audits and validated backlog follow-ups.
- `.agents/skills/tdd/SKILL.md` and `.agents/skills/refactor/SKILL.md` — implementation and behavior-preserving cleanup workflows.
- `.agents/skills/grill-me/SKILL.md` — resolve material ambiguity or conflicting requirements.
- `.agents/skills/clean-code-principles/SKILL.md` — design principles and implemented rules.

---

## 1. Prime Directives & Constraints

### ⚠️ DIRECTIVE 1: Python-Only Repository Automation via `doit` (Strict)
* **All repository-wide automation MUST be written in Python 3.12 (`automation/*.py`) and orchestrated through `doit` (`automation/dodo.py` + `doit.cfg`).**
* **Shell scripts (`.sh` / bash) are strictly forbidden** for repo automation, build helpers, release scripts, or code generation.
* **The sole permitted file in `scripts/`** is `scripts/restore.sh`, which `pixi-sandbox init` generates and `pixi run sandbox-check` verifies. Alint (`forbid-non-restore-scripts` in `.alint.yml`) and `automation/repo_check.py` reject every other file under `scripts/`; place all other helpers in `automation/`.
* Domain modules inside `automation/*.py` must remain pure Python modules (`main(argv) -> int`) that never import `doit`, never invoke `subprocess` with `shell=True`, always pass explicit timeouts, and carry unit tests under `automation/tests/`.

### ⚠️ DIRECTIVE 2: Hermetic Single-Manifest Toolchain Execution
* **Every command must run through the single root `pixi.toml`:** `pixi run --frozen <task>` for the `default` environment or `pixi run --frozen -e <env> <task>` for an isolated SDK environment (`py`, `ts`, `cpp`, `c`, `csharp`, `go`, `java`, `r`).
* Do **not** re-introduce per-directory `<lang>/pixi.toml` or `<lang>/pixi.lock` files; Alint enforces a single root Pixi workspace.
* Never invoke bare system binaries (`cargo`, `pytest`, `bun`, `ruff`, `go`, `javac`, `Rscript`) directly; always run through Pixi to guarantee reproducible dependency isolation.

### ⚠️ DIRECTIVE 3: Conventional Commits & Clean Code Governance
* All commit messages must adhere to the **Conventional Commits** specification configured in `.versionrc`.
* Allowed types: `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `ci`, `chore`, `build`, `style`, `revert`.
* Allowed scopes: `rs`, `py`, `ts`, `cpp`, `c`, `csharp`, `go`, `java`, `r`, `core`, `fixtures`, `bdd`, `pbt`, `vcr`, `snap`, `cli`, `deps`, `release`, `scripts`, `automation`, `clean-code`, `backlog`.
* Vendored presets in `vendor/clean-code-presets/` (`Archont561/polyglot-clean-code` pinned in `UPSTREAM.pin` and checksummed in `VENDOR.sha256`) plus `rude-clean-code` (`rude.toml`) enforce `PCC-R*`, `PCC-P*`, `PCC-S*`, `PCC-D*`, and `CC0`–`CC9` on every run of `pixi run do clean_code`.
* Python Clean Code is a lint gate, not an advisory: `pixi run lint` and `pixi run lint-automation` run `python -m automation.rude_clean`, the `py` SDK lane runs it from `sdk-lint`, and the `pre-commit` hook runs it on every `*.py` change through the `python-format-lint` job. Any `error`, `warning`, or `info` diagnostic fails the gate.
* Validate commit messages before pushing:
  ```bash
  echo "feat(fixtures): add async teardown" | pixi run convco check --from-stdin
  ```

---

## 2. Release Lifecycle & Git Tagging Architecture

The repository enforces a strict separation of concerns across GitHub Actions workflows (every workflow declares top-level `permissions: contents: read`, pins third-party actions to 40-character commit SHAs, and keeps custom `run:` steps to a single command):

```
[ Milestone tag push / workflow_dispatch ]
                   │
                   ▼
     .github/workflows/auto-release.yml
     (Change detection via automation/release.py)
     (Runs quality gates & dry-run packaging)
     (Cuts & pushes annotated release tag via RELEASE_PUSH_TOKEN or dry-run summary)
                   │
                   ▼
       .github/workflows/release.yml
       (Phase 1: Stamp unified version, build packages & upload artifacts)
       (Phase 2: Publish to registries & attach GitHub Release assets)
```

### Workflow Specifications

| Workflow | Path | Trigger | Core Responsibility |
| :--- | :--- | :--- | :--- |
| **CI** | `.github/workflows/ci.yml` | Pull Requests, `main` pushes, dispatch | Runs `fast` Clean Code/workflow/automation gates, `change_scope` path filtering, Rust workspace clippy/nextest/doc/package checks, affected SDK lanes, Codecov merge, and the required `gates` status check. **Never cuts releases or tags.** |
| **Security** | `.github/workflows/security.yml` | Weekly schedule, dependency PRs, dispatch | Runs `cargo deny check` and live `cargo audit` against the Advisory Database (complementing offline `audit-local`). |
| **Relock** | `.github/workflows/relock.yml` | Pull Requests touching manifests/workflows | Generated by `pixi-sandbox`; refreshes `pixi.lock` + `Cargo.lock` as `pixi-sandbox[bot]` and validates downstream workflows. |
| **Publish Sandbox** | `.github/workflows/publish-sandbox.yml` | `main` pushes touching sandbox inputs, dispatch | Generated by `pixi-sandbox`; packs `default` Pixi env + vendored Rust crates to `sandbox/developer-linux-64`. |
| **Auto-Release** | `.github/workflows/auto-release.yml` | Milestone tag push (`milestone-v*`, `milestone/*`, `m-v*`) or dispatch | Runs `automation/release.py`, validates quality gates, and pushes the annotated release tag `vX.Y.Z` when `RELEASE_PUSH_TOKEN` is configured (otherwise emits the exact tag command in `$GITHUB_STEP_SUMMARY`). |
| **Release** | `.github/workflows/release.yml` | Release tag push (`v*`, excluding milestone tags) or dispatch | **Phase 1**: Verifies version parity (`gate`), stamps unified version across all 5 crates and 8 SDKs via `automation/stamp_version.py`, builds multi-platform CLI archives, wheels, npm tarballs, `.conda` packages, and SDK packages, and uploads them.<br>**Phase 2**: Publishes Rust (`playtest-macros` $\rightarrow$ `playtest-core` $\rightarrow$ `playtest-assets` $\rightarrow$ `playtest` $\rightarrow$ `playtest-cli`), Python, TypeScript, conda, C/C++ (Conan), C# (NuGet), Java (Maven Central), R (`--as-cran`), immutable GitHub Release assets, Go (`go/vX.Y.Z` post-release), and R-universe check. |
| **CRAN Submission** | `.github/workflows/cran-publish.yml` | Manual dispatch | Select a release tag and enter `CONFIRM`; validates and submits the R package, then CRAN emails the maintainer for confirmation. |

### Milestone Tag Pushes
* Maintainers can trigger automated release execution by pushing a milestone tag:
  ```bash
  git tag -a "milestone-v0.2.0" -m "Milestone v0.2.0 release trigger"
  git push origin "milestone-v0.2.0"
  ```
* `auto-release.yml` intercepts the milestone tag, extracts target version `0.2.0`, runs quality gates and dry-run packaging, and pushes the final unified release tag `v0.2.0` when `RELEASE_PUSH_TOKEN` is configured (or records the exact tag command in `$GITHUB_STEP_SUMMARY`).

### Unified Release Tags (`vX.Y.Z`) & Go Subdirectory Tag (`go/vX.Y.Z`)
* **Unified Releases (`vX.Y.Z`)**: Per ADR 0016, a release cuts one version `vX.Y.Z` across all 5 Rust crates and all 8 language SDKs (`automation/release.py` + `automation/stamp_version.py`).
* **Go Subdirectory Module Tag (`go/vX.Y.Z`)**: Go resolves the module in `go/` only from tags carrying the `go/` subdirectory prefix (`automation/publish_go.py` pushes `go/vX.Y.Z` at the same commit as `vX.Y.Z` and requests `proxy.golang.org` indexing).
* Legacy scoped tags (`<scope>-vX.Y.Z`) are retired from automated cutting and workflow tag globs so `milestone-v*` / `m-v*` never collide with `release.yml`.

### Registry Configuration
* The `crates` GitHub environment is bound via crates.io OIDC Trusted Publishing (`Archont561/playtest`, workflow `release.yml`, environment `crates`) across all five reserved crates (`playtest`, `playtest-macros`, `playtest-core`, `playtest-assets`, `playtest-cli`). `publish-crates` exchanges the GitHub OIDC JWT via `rust-lang/crates-io-auth-action@v1` (`id: auth`) and passes `CARGO_REGISTRY_TOKEN: ${{ steps.auth.outputs.token }}` to `pixi run --frozen publish-rs`.
* The `pypi` and `npm` GitHub environments publish via OIDC Trusted Publishing (`pypa/gh-action-pypi-publish` and `npm publish --provenance --access public`).
* The `conda` GitHub environment uploads `.conda` artifacts to `https://prefix.dev/archont561/archont561` when `PREFIX_DEV_TOKEN` is configured.
* Optional secret-authenticated registries are skipped with a warning when their credentials are not configured; GitHub Release artifacts remain the fallback.
* The `conan` GitHub environment must define `CONAN_REMOTE_URL`, `CONAN_USERNAME`, and `CONAN_PASSWORD`. The workflow publishes the `playtest-c` and/or `playtest-cpp` recipes to that Conan 2 remote.
* The `nuget` environment must define `NUGET_API_KEY` for NuGet.org publishing.
* The `maven` environment must define `MAVEN_CENTRAL_USERNAME`, `MAVEN_CENTRAL_PASSWORD`, `MAVEN_GPG_PRIVATE_KEY`, and `MAVEN_GPG_PASSPHRASE`. Maven Central publishing requires an approved namespace and signing key.
* The `cran` environment must define `AUTHOR_EMAIL` as a variable or secret. The release workflow stamps it into the R package metadata and runs `R CMD check --as-cran`. To submit, manually dispatch `.github/workflows/cran-publish.yml` with the release `tag` and `confirmation=CONFIRM`; CRAN still emails the maintainer address for final confirmation.
* R-universe is an automatic source-based deployment, not an artifact upload. To enable the optional post-release verification job, set repository variable `RUNIVERSE_ENABLED=true` (and optionally `RUNIVERSE_OWNER`). Create `<owner>.r-universe.dev` with a `packages.json` entry for `playtest` tracking `branch: "*release"`, and install the R-universe GitHub app on that owner. The job waits for the public package API to report the released version. R-universe builds from the source at the GitHub release tag, so that tagged source must already contain the matching `DESCRIPTION` version (the CI-time version stamp is not committed back to the tag).

### CLI-First Distribution Invariants (ADRs 0015–0018)

Milestone M1 (`task_050` to `task_059`) implements the five-crate workspace, `playtest` CLI (`version`, `init`, `sync`), deterministic Python/TypeScript mirrors, and multi-channel release pipeline; Milestone M5 (`task_060` to `task_064`) delivers runner orchestration (`list`, `doctor`, `test`, `mutation`) and SDK JSONL reporters:

* **One version per release.** The Rust CLI `playtest` is the version authority and embeds every SDK. A release has one version across the CLI and all SDKs. Never bump the version of one package alone.
* **Package-scoped tags are retired.** Only unified `vX.Y.Z` tags trigger releases (`automation/release.py`), and `publish-go` pushes `go/vX.Y.Z` after `github-release` publishes `sha256.sum`.
* **Five crates.** `playtest`, `playtest-macros`, `playtest-core`, `playtest-assets`, and `playtest-cli` (Spec 0012 §1). `playtest` must never depend on `playtest-assets`, `playtest-cli`, or `clifacet`.
* **Binary channels.** GitHub Release archives, crates.io, PyPI wheels, npm platform packages, conda, and Go and R download wrappers (ADR 0016). No package may download anything at install time or declare an npm lifecycle script.

---

## 3. Backlog Management & Semantic Versioning Rules

The `backlog/` directory is the single source of truth for planning, architecture, specifications, and execution:

```
backlog/
├── config.toml           # Backlog tracking and branch configuration
├── tasks.yaml            # Machine-readable tasks bound to milestones and SemVer
└── docs/
    ├── milestones/       # Milestone definitions with explicit SemVer bindings
    │   ├── M1-v0.1.0-foundation.md             # Reopened: CLI binary, channels, init/sync
    │   ├── M2-v0.2.0-core-fixtures-and-macros.md
    │   ├── M3-v0.3.0-fluent-bdd-and-pbt.md
    │   ├── M4-v0.4.0-isolation-and-vcr-cassettes.md
    │   ├── M5-v0.5.0-polyglot-conformance.md   # Includes CLI orchestration, Doctor & SDK reporters
    │   ├── M6-v1.0.0-universal-stable-release.md
    │   ├── M7-v1.1.0-ecosystem-integration-fixtures.md
    │   └── README.md
    ├── adr/              # Architectural Decision Records (ADRs 0001 - 0019)
    └── specs/            # Formal technical specifications (Specs 0001 - 0014)
```

Five more files in `milestones/` (`M1-v0.1.0-fixture-engine.md`, `M2-v0.2.0-macro-attributes.md`, `M3-v0.3.0-fluent-bdd.md`, `M4-v0.4.0-standard-isolation-fixtures.md`, `M5-v1.0.0-production-release.md`) belong to an earlier Rust-only roadmap. They carry a "Superseded" banner. Do not plan from them.

`backlog/docs/adr/README.md` gives the status of every ADR. A Proposed ADR is not a decision: ADR 0012, 0013, 0014, and 0019 are Proposed. An accepted ADR is changed by an appended Amendments section or by a new ADR, never by rewriting its Decision.

### Task Synchronization Directives
1. **SemVer Binding**: Every task in `backlog/tasks.yaml` must explicitly declare:
   * `milestone: "M<ID>-v<SEMVER>"` (e.g. `M2-v0.2.0`)
   * `semver: "<SEMVER>"` (e.g. `"0.2.0"`)
2. **Status Progression**:
   * `backlog` $\rightarrow$ `ready` $\rightarrow$ `in_progress` $\rightarrow$ `done`.
   * Tasks must never be deleted; update status and acceptance criteria.
3. **CI Path Filtering**:
   * Documentation and backlog task updates (`backlog/**`, `*.md`) are ignored by release change detection and full test matrix runs to conserve compute resources.

---

## 4. Polyglot Architecture & Parity Requirements

Playtest's parity target covers all nine language targets: Rust, Python, TypeScript, C++, C, C#, Go, Java, and R. **This is the required behavioral contract, not a claim that every target is already implemented or verified.** Use `backlog/docs/specs/0005-polyglot-1-to-1-fixture-and-bdd-parity.md` as the source of truth and check the milestone/task status before asserting support.

Across targets, the contract requires fixture injection and documented scopes, requested-only resolution, failure-safe LIFO teardown, explicit-state Given/When/Then/And scenarios, property testing with fresh per-sample fixtures and reproducible shrinking, skip/parameterized cases, and equivalent isolation/determinism helpers from Spec 0004. Public APIs and test libraries may be idiomatic; observable behavior must match and pass the shared conformance suite.

The cross-language `playtest` CLI is one shared orchestrator, not a separate CLI requirement for each language. It must detect and run all nine targets and report test/coverage outcomes consistently. Do not treat toolchain availability or a successful aggregate command as proof of feature parity.

Rules that follow from ADRs 0015 to 0018. They bind new work now, although the CLI is not implemented yet:

* **No CLI code in a language package.** The only per-language CLI code is a wrapper that locates or downloads the one binary. Do not add a CLI framework to a language package. `typer` (Python), `commander` (TypeScript), and `cobra` (Go) are declared today, are used by no code, and are removed by `task_055`, `task_056`, and `task_058`.
* **The CLI is `crates/playtest-cli`**, built on `clifacet`. Keep `clifacet` types out of documented output; the output and exit-status contract is Spec 0008 §7.
* **SDKs report; they do not run.** An SDK attaches a reporter to its native runner and writes records only when the CLI switches it on (Spec 0014). Do not add a runner entry point such as `python -m playtest` to an SDK.
* **`playtest init` and `playtest sync` write only `playtest.toml` and `.playtest/`.** They never edit a consumer's manifest or lockfile (Spec 0013).
* **Version skew is an error**, reported with exit status 3. Do not add a compatibility mode between versions.

---

## 5. Verification Checklist for Agents

Before completing any task or ending a turn, agents must verify that all quality gates pass:

```bash
# 1. Run Clean Code, structure, workflow, TOML, and automation gates (includes rude-clean-code)
pixi run do fast

# 2. Run automation unit tests and strict mypy checks
pixi run do test_automation

# 3. Check formatting across all targets
pixi run fmt-check

# 4. Run offline Rust workspace gates (clippy, nextest, doc, cargo-deny)
pixi run offline-rs
pixi run audit-local

# 5. Run SDK test suites for affected targets (or all targets)
pixi run test-all

# 6. Verify packaging dry-runs across ecosystems
pixi run package-all
```
