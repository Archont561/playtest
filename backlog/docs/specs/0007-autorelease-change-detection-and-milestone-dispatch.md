# Specification 0007: Auto-Release Change Detection and Milestone Dispatch

## Overview
Specifies the change detection algorithm, Conventional Commits version resolution, milestone tag processing, and release dispatch workflows implemented across `scripts/detect_release.py` and `.github/workflows/auto-release.yml`.

## Trigger Specifications

The auto-release workflow executes under three conditions:
1. **Push to `main` Branch**:
   - Monitored for commits on `main`.
   - Paths ignored: `backlog/**`, `.agents/**`, `.devcontainer/**`, `*.md`, `.codecov.yml`.
2. **Milestone Tag Push**:
   - Monitored tags matching pattern: `milestone-v*`, `milestone/*`, `m-v*`.
   - Example: `git push origin milestone-v0.2.0`.
3. **Manual Dispatch (`workflow_dispatch`)**:
   - Supports inputs: `scope` (override target), `bump` (override SemVer increment), `dry-run` (validate without pushing tags).

## Detection & Calculation Algorithm (`scripts/detect_release.py`)

```python
# 1. Base Reference Resolution
latest_global_tag = git tag -l "v[0-9]*" --sort=-v:refname | head -n 1
root_commit = git rev-list --max-parents=0 HEAD | head -n 1
base_ref = latest_global_tag or root_commit

# 2. File Categorization via git diff
changed_files = git diff --name-only base_ref..HEAD

# 3. Scope Evaluation
if core_changed or active_targets > 1:
    scope = "all"        # Prefix: "v"
elif active_targets == 1:
    scope = target_name  # Prefix: "<target>-v" (e.g. "python-v")
else:
    scope = "none"       # No release needed

# 4. Version Calculation via convco
if milestone_tag:
    version = extract_semver(milestone_tag)
else:
    version = convco version --bump --prefix "$prefix" --treat-major-zero-as-stable
```

## Tag Generation Rules

| Scope | Computed Prefix | Computed Version | Generated Release Tag |
| :--- | :--- | :--- | :--- |
| `all` | `v` | `0.2.0` | `v0.2.0` |
| `rust` | `rust-v` | `0.2.0` | `rust-v0.2.0` |
| `python` | `python-v` | `0.2.0` | `python-v0.2.0` |
| `typescript` | `ts-v` | `0.2.0` | `ts-v0.2.0` |
| `cpp` | `cpp-v` | `0.2.0` | `cpp-v0.2.0` |
| `go` | `go-v` | `0.2.0` | `go-v0.2.0` |
| `java` | `java-v` | `0.2.0` | `java-v0.2.0` |
| `r` | `r-v` | `0.2.0` | `r-v0.2.0` |

Pushing the release tag from `auto-release.yml` automatically triggers `.github/workflows/release.yml`.
