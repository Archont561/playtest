# Specification 0007: Auto-Release Change Detection and Milestone Dispatch

## Overview
Specifies the unified change detection algorithm, Conventional Commits version resolution, milestone tag processing, and release dispatch workflows implemented across `automation/release.py`, `.github/workflows/auto-release.yml`, `.github/workflows/release.yml`, and `.github/workflows/cran-publish.yml` ([ADR 0010](../adr/0010-milestone-driven-autorelease-lifecycle.md), [ADR 0015](../adr/0015-cli-first-distribution-and-five-crate-workspace.md)).

## Trigger Specifications

The auto-release workflow (`.github/workflows/auto-release.yml`) evaluates three trigger paths:
1. **Push to `main` Branch**:
   - Ignores changes confined to `backlog/**`, `.agents/**`, `.devcontainer/**`, `*.md`, and `.codecov.yml`.
   - **First-release guard:** When no `v[0-9]*` tag exists in the repository yet, pushes to `main` always resolve to `should_release=false` so `v0.1.0` cannot be auto-tagged by accident. The initial release must be cut via a milestone tag (`milestone-v0.1.0`) or `workflow_dispatch` `override_version`.
2. **Milestone Tag Push**:
   - Monitored tags matching `milestone-v*`, `milestone/*`, or `m-v*` (for example, `git push origin milestone-v0.1.0`).
3. **Manual Dispatch (`workflow_dispatch`)**:
   - Inputs: `override_version` (explicit target SemVer) and `dry_run` (validate gates and emit a step summary without pushing a tag).

## Detection & Calculation Algorithm (`automation/release.py`)

```python
# 1. Explicit Milestone or Manual Version
if override_version:
    return {"should_release": "true", "version": override_version, "tag_name": f"v{override_version}"}
if milestone_tag:
    ver = parse_milestone_tag(ref_name)
    return {"should_release": "true", "version": ver, "tag_name": f"v{ver}"}

# 2. First-Release Guard on Push to main
last_tag = git describe --tags --match "v[0-9]*" --abbrev=0
if not last_tag:
    return {"should_release": "false", "version": workspace_version, "tag_name": f"v{workspace_version}"}

# 3. Unified Monorepo Diff & Conventional Commits Bump
releasable_files = [f for f in git_diff_names(f"{last_tag}..HEAD") if is_releasable_path(f)]
if not releasable_files:
    return {"should_release": "false", "version": base_ver, "tag_name": last_tag}

next_ver, should_bump = bump_semver_from_commits(base_ver, git_log_subjects(f"{last_tag}..HEAD"))
return {"should_release": str(should_bump).lower(), "version": next_ver, "tag_name": f"v{next_ver}"}
```

## Unified Tag Policy (Scoped Tags Retired — `task_059`)

| Tag Form | Role | Accepted by `release.yml` / `cran-publish.yml`? |
| :--- | :--- | :--- |
| `vX.Y.Z` | Canonical unified release tag across CLI and all SDKs | Yes (sole release trigger; CRAN additionally requires non-prerelease `vX.Y.Z` with published binaries) |
| `go/vX.Y.Z` | Output Go module subdirectory alias pushed by `publish-go` **after** `github-release` publishes `vX.Y.Z` binaries | No — rejected as a standalone release trigger |
| `<scope>-vX.Y.Z` (`python-v*`, `rust-v*`, `r-v*`, …) | Retired (`ADR 0010`, `ADR 0015`) | No — rejected by `plan_release_tag()` and `validate_cran_tag()` |

Rationale settled by `task_059`:
1. The CLI embeds every SDK ([Spec 0012 §2](./0012-workspace-crates-and-cli-distribution.md)), so even a change confined to a single language directory alters the CLI binary and requires a unified `vX.Y.Z` release.
2. Both the Go download wrapper (`go/cmd/playtest`) and `playtest::install_cli()` fetch `vX.Y.Z` archives and `sha256.sum` from GitHub Releases ([Spec 0012 §5](./0012-workspace-crates-and-cli-distribution.md)). Retiring standalone `go/vX.Y.Z` and `r-vX.Y.Z` triggers — and verifying `vX.Y.Z/sha256.sum` over HTTPS inside `automation/publish_go.py` and `automation/release.py validate-cran-tag` — guarantees no Go or R version can ever be published without its matching binary release.

## Workflow Chaining Under `GITHUB_TOKEN`

GitHub Actions does not start `on: push: tags:` workflows for tags pushed using the default `GITHUB_TOKEN`. To make the tag-to-release chain work end to end without requiring a Personal Access Token:
- When `secrets.RELEASE_PUSH_TOKEN` is configured, pushing `refs/tags/vX.Y.Z` triggers `release.yml` directly via `on: push: tags:`.
- When `secrets.RELEASE_PUSH_TOKEN` is absent, `auto-release.yml` pushes `refs/tags/vX.Y.Z` using `GITHUB_TOKEN` (`contents: write`) and immediately dispatches `release.yml` on `refs/tags/vX.Y.Z` with `inputs: { tag: "vX.Y.Z" }` via `actions/github-script` (`actions: write`).
