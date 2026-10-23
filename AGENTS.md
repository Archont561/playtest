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

### ⚠️ DIRECTIVE 1: Python-Only Repository Automation (Strict)
* **All repository-wide automation scripts MUST be written in Python 3 (`.py`).**
* **Shell scripts (`.sh` / bash) are strictly forbidden** for repo automation, build helpers, release scripts, or code generation.
* **The sole permitted exception** is `scripts/restore.sh`, which is vendor-provided by `pixi-sandbox` and part of the sandbox restoration contract.
* Any new workflow helper, generator, or detection script must reside in `scripts/*.py` and be invoked via `pixi run python ...`.

### ⚠️ DIRECTIVE 2: Hermetic Toolchain Execution
* **Every command must run through Pixi:** `pixi run <task>` or `pixi run --manifest-path <subpkg>/pixi.toml <task>`.
* Never invoke bare system binaries (`cargo`, `pytest`, `bun`, `ruff`, `go`, `javac`, `Rscript`) directly; always use the hermetic Pixi environment to guarantee reproducible dependency isolation.

### ⚠️ DIRECTIVE 3: Conventional Commits & Versioning
* All commit messages must strictly adhere to the **Conventional Commits** specification configured in `.versionrc`.
* Allowed types: `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `ci`, `chore`.
* Allowed scopes: `rs`, `py`, `ts`, `cpp`, `c`, `csharp`, `go`, `java`, `r`, `core`, `fixtures`, `bdd`, `pbt`, `vcr`, `snap`, `cli`, `deps`, `release`, `scripts`, `backlog`.
* Validate commit messages before pushing:
  ```bash
  echo "feat(fixtures): add async teardown" | pixi run convco check --from-stdin
  ```

---

## 2. Release Lifecycle & Git Tagging Architecture

The repository enforces a strict, three-tier separation of concerns across GitHub Actions workflows:

```
[ Push to main / milestone tag ]
               │
               ▼
   .github/workflows/auto-release.yml
   (Change detection via python scripts/detect_release.py)
   (Runs quality gates & dry-run packaging)
   (Cuts & pushes proper release tag)
               │
               ▼
   .github/workflows/release.yml
   (Phase 1: Build packages & upload GitHub Actions artifacts)
   (Phase 2: Downstream jobs download artifacts & publish to registries)
```

### Workflow Specifications

| Workflow | Path | Trigger | Core Responsibility |
| :--- | :--- | :--- | :--- |
| **CI** | `.github/workflows/ci.yml` | Pull Requests & non-tag pushes | Validates code health: lints, formatting, multi-target tests, 90% Codecov coverage, vulnerability audit. **Never cuts releases or tags.** |
| **Auto-Release** | `.github/workflows/auto-release.yml` | Push to `main` (ignoring `backlog/**`, `*.md`) OR **Milestone tag push** (`milestone-v*`, `milestone/*`, `m-v*`) OR dispatch | Runs `python scripts/detect_release.py`, validates quality gates, and cuts the proper annotated release tag (`v*` or `<scope>-v*`). |
| **Release** | `.github/workflows/release.yml` | Release tag push (`v*`, `*-v*`, `*/*v*`) OR dispatch | **Phase 1**: Builds `.crate`, `.whl`, `.tgz`, `.jar`, `.tar.gz` into GitHub artifacts.<br>**Phase 2**: Downstream publish jobs upload artifacts to crates.io, PyPI, npm, Maven, Go proxy, Conan, CRAN, and GitHub Releases. |

### Milestone Tag Pushes
* Maintainers can trigger automated release execution by pushing a milestone tag:
  ```bash
  git tag -a "milestone-v0.2.0" -m "Milestone v0.2.0 release trigger"
  git push origin "milestone-v0.2.0"
  ```
* `auto-release.yml` intercepts the milestone tag, extracts target version `0.2.0`, runs quality gates and dry-run packaging, and pushes the final release tag `v0.2.0` (or `<scope>-v0.2.0`).

### Scoped vs. General Tags
* **General Releases (`vX.Y.Z`)**: Triggered when root build files (`pixi.toml`), shared infrastructure, or $>1$ target changes. Triggers publishing across **all 9 targets**.
* **Scoped Releases (`<scope>-vX.Y.Z`)**: Triggered when only a single target changes:
  * Rust: `rust-vX.Y.Z` $\rightarrow$ crates.io
  * Python: `python-vX.Y.Z` $\rightarrow$ PyPI
  * TypeScript: `ts-vX.Y.Z` $\rightarrow$ npm
  * C++: `cpp-vX.Y.Z` $\rightarrow$ GitHub Release / Conan
  * C: `c-vX.Y.Z` $\rightarrow$ GitHub Release / Conan / tarball
  * C#: `csharp-vX.Y.Z` $\rightarrow$ NuGet / GitHub Packages
  * Go: `go-vX.Y.Z` $\rightarrow$ Go Proxy / `pkg.go.dev`
  * Java: `java-vX.Y.Z` $\rightarrow$ Maven Central / GitHub Release
  * R: `r-vX.Y.Z` $\rightarrow$ CRAN / GitHub Release

---

## 3. Backlog Management & Semantic Versioning Rules

The `backlog/` directory is the single source of truth for planning, architecture, specifications, and execution:

```
backlog/
├── config.toml           # Backlog tracking and branch configuration
├── tasks.yaml            # Machine-readable tasks bound to milestones and SemVer
└── docs/
    ├── milestones/       # Milestone definitions with explicit SemVer bindings
    │   ├── M1-v0.1.0-foundation.md
    │   ├── M2-v0.2.0-core-fixtures-and-macros.md
    │   ├── M3-v0.3.0-fluent-bdd-and-pbt.md
    │   ├── M4-v0.4.0-isolation-and-vcr-cassettes.md
    │   ├── M5-v0.5.0-polyglot-conformance.md   # Includes Cross-Lang CLI & Doctor
    │   ├── M6-v1.0.0-universal-stable-release.md
    │   └── README.md
    ├── adr/              # Architectural Decision Records (ADRs 0001 - 0011)
    └── specs/            # Formal technical specifications (Specs 0001 - 0008)
```

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

---

## 5. Verification Checklist for Agents

Before completing any task or ending a turn, agents must verify that all quality gates pass:

```bash
# 1. Lint manifests and configurations
pixi run lint-toml

# 2. Check code formatting across all targets
pixi run fmt-check

# 3. Run all target test suites
pixi run test-all

# 4. Verify GitHub Actions workflow syntax
pixi run actionlint

# 5. Run release change detection dry-run
pixi run detect-release

# 6. Verify packaging for all language targets
pixi run package-all
```
