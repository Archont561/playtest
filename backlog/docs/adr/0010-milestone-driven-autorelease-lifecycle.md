# ADR 0010: Milestone-Driven Auto-Release Lifecycle

## Status
Accepted. Amended on 2026-11-11 for the Go tag form. [ADR 0015](./0015-cli-first-distribution-and-five-crate-workspace.md) puts the scoped tags of this record under review (`task_059`). See Amendments below.

## Context
Releasing multi-language software requires a predictable mechanism to transition planned development milestones into tagged, published deliverables. Relying solely on manual git tagging or commits directly to release branches risks publishing untested code or misformatting tags. Maintainers need the ability to push a milestone tag or trigger auto-release on push to `main` while guaranteeing strict validation.

## Decision
We established a milestone-driven release lifecycle connecting git milestone tags to automated publishing:
1. **Milestone Tag Convention**:
   - Pushing a tag matching `milestone-v<SEMVER>` or `milestone/v<SEMVER>` (e.g., `milestone-v0.2.0`) automatically triggers `.github/workflows/auto-release.yml`.
2. **Automated Change & Scope Detection**:
   - The workflow invokes `python scripts/detect_release.py`.
   - If triggered by a milestone tag, the target SemVer version is bound directly to that milestone.
   - If triggered by a regular push to `main`, `scripts/detect_release.py` inspects git diffs against the last release tag and queries `convco` to determine whether a general tag (`v*`) or package-scoped tag (`<scope>-v*`) is warranted.
3. **Automated Quality Gate & Tag Publication**:
   - Enforces formatting, linter, test, and package build checks.
   - Creates and pushes the clean release tag (`vX.Y.Z` or `<scope>-vX.Y.Z`), which in turn triggers `.github/workflows/release.yml` for remote publishing.

## Consequences
### Positive
- Clear audit trail linking backlog milestones directly to published package versions.
- Pushing a single milestone tag initiates the entire quality validation and deployment pipeline hands-free.
- Prevents release of untracked or misaligned versions.

### Negative
- Requires maintaining milestone tags in git alongside release tags.

## Amendments

**2026-11-11.**

- The scoped tag for Go is `go/vX.Y.Z`, not `go-vX.Y.Z` ([Spec 0007](../specs/0007-autorelease-change-detection-and-milestone-dispatch.md)).
- [ADR 0015](./0015-cli-first-distribution-and-five-crate-workspace.md) gives every release one version across the CLI and all SDKs. Package-scoped tags release one language at its own version, which conflicts with that. `task_059` decides their future. Until then this record describes the implemented behavior.
