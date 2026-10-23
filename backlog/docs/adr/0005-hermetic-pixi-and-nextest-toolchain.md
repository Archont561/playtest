# ADR 0005: Hermetic Toolchain with Pixi, Lefthook, and Nextest

## Context
Toolchain drift between developer machines, offline sandboxes, and CI runners causes subtle verification failures (e.g. mismatched Clippy versions, missing shellcheck binaries).

## Decision
1. Standardize on **Pixi** (`conda-forge` + `prefix.dev/archont561`) to lock exact compiler and tool versions.
2. Delegate all CI steps and Git hooks to `pixi run --frozen <task>`.
3. Use **Lefthook** + **convco** to enforce Conventional Commits and pre-commit checks locally.
4. Use **`cargo-nextest`** as the default test runner for faster execution and structured output.
5. Provide offline sandbox restoration via `pixi-sandbox` and `scripts/restore.sh`.

## Consequences
- Single source of truth in `pixi.toml`.
- Airgapped execution ready without root/sudo access.
