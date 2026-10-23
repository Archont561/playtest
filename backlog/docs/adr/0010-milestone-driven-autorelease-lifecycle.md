# ADR 0010: Milestone-Driven Auto-Release Lifecycle

## Status
Accepted

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
