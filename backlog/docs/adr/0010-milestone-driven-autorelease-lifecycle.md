# ADR 0010: Milestone-Driven Auto-Release Lifecycle

## Status
Accepted. Amended on 2026-11-11 to retire package-scoped release tags in favor of one unified `vX.Y.Z` version per release ([ADR 0015](./0015-cli-first-distribution-and-five-crate-workspace.md), `task_059`).

## Context
Releasing multi-language software requires a predictable mechanism to transition planned development milestones into tagged, published deliverables. Relying solely on manual git tagging or commits directly to release branches risks publishing untested code or misformatting tags. Maintainers need the ability to push a milestone tag or trigger auto-release on push to `main` while guaranteeing strict validation and keeping the CLI binary and every language SDK at the exact same SemVer release.

## Decision
We established a milestone-driven release lifecycle connecting git milestone tags and `main` branch changes to unified `vX.Y.Z` releases:
1. **Milestone Tag Convention**:
   - Pushing a tag matching `milestone-v<SEMVER>`, `milestone/M<N>-v<SEMVER>`, `milestone/v<SEMVER>`, or `m-v<SEMVER>` (e.g., `milestone-v0.1.0`) automatically triggers `.github/workflows/auto-release.yml`.
2. **Unified Change Detection & First-Release Guard**:
   - The workflow invokes `python -m automation.release detect`.
   - If triggered by a milestone tag or `workflow_dispatch` `override_version`, the target SemVer version is bound directly to that version.
   - If triggered by a push to `main` when **no prior `v[0-9]*` tag exists**, `detect` returns `should_release=false` so an ordinary push to `main` never tags `v0.1.0` accidentally; the initial `v0.1.0` release requires an explicit milestone tag or `override_version`.
   - Once `v0.1.0` exists, any releasable Conventional Commit (`feat`, `fix`, `perf`, or `!:` / `BREAKING CHANGE`) touching non-documentation paths — even when confined to a single language directory — cuts a **unified `vX.Y.Z`** release across the CLI and every SDK.
   - Package-scoped release tags (`<scope>-vX.Y.Z` and standalone `go/vX.Y.Z`) are **retired**. `go/vX.Y.Z` is pushed solely as a Go module subdirectory alias by `publish-go` strictly *after* `github-release` publishes `vX.Y.Z` with verified binary archives.
3. **Automated Quality Gate, Tag Publication, and Workflow Chaining**:
   - Enforces formatting, Clean Code, polyglot test, and package build checks before cutting the annotated `vX.Y.Z` tag.
   - Because GitHub suppresses `on: push: tags:` triggers for events pushed with `GITHUB_TOKEN`, `auto-release.yml` (`permissions: contents: write, actions: write`) explicitly dispatches `release.yml` via `workflow_dispatch` on `refs/tags/vX.Y.Z` whenever `RELEASE_PUSH_TOKEN` is not configured.

## Consequences
### Positive
- Every published release keeps the CLI binary, embedded SDK mirrors, download wrappers, and all registry packages at one exact version.
- No Go or R release can ever exist without a verified binary release of the same version on GitHub Releases.
- `auto-release.yml` reliably starts `release.yml` under default `GITHUB_TOKEN` permissions without requiring a personal access token.

### Negative
- A single-language fix increments the version number of the entire monorepo (`task_066` settles post-`1.0` freeze semantics).

## Amendments

**2026-11-11 (`task_059`).**

- Retired all package-scoped release tags (`<scope>-vX.Y.Z`, `r-vX.Y.Z`, and standalone `go/vX.Y.Z` release triggers). `plan_release_tag()` and `validate_cran_tag()` accept only unified `vX.Y.Z` tags (`Spec 0007`, `Spec 0012 §8`).
- `go/vX.Y.Z` remains only as the output subdirectory module tag created by `publish-go` after `github-release` attaches the `vX.Y.Z` CLI archives and `sha256.sum`.
- Added the first-release guard in `detect_next_release()` and explicit `workflow_dispatch` chaining from `auto-release.yml` to `release.yml` when pushing tags with `GITHUB_TOKEN`.
