# ADR 0007: Polyglot Monorepo Architecture via Pixi Workspaces

## Status
Accepted

## Context
While `playtest` originated as a Rust-centric test runner, unified testing ergonomics across modern systems and services require seamless interoperability across multiple programming languages. To achieve 1:1 capability parity between Rust, Python, and TypeScript (alongside compiled and specialized targets in C++, Go, Java, and R), the repository needed a cohesive multi-package monorepo architecture. Managing multiple disparate language package managers without version collision or drift introduced substantial friction.

## Decision
We adopted a unified polyglot monorepo structure orchestrated by **Pixi** (`pixi.toml`):
1. **Root Pixi Workspace**: Root `pixi.toml` pins shared developer toolchains (`convco`, `actionlint`, `taplo`, `cargo-nextest`, `cargo-llvm-cov`, `cargo-audit`, `ruff`, `bun`, `openjdk`, `r-base`, `go`, `gcc`).
2. **Subpackage Manifests**: Each language target resides in its own isolated directory (`crates/`, `python/`, `typescript/`, `cpp/`, `go/`, `java/`, `r/`) with a package-scoped `pixi.toml` defining local build, lint, test, and release tasks.
3. **Delegation Tasks**: Root `pixi.toml` exposes compound tasks (`lint-all`, `test-all`, `build-all`, `package-all`, `release-all`) that execute across all targets via `--manifest-path`.

## Consequences
### Positive
- Single hermetic environment setup (`pixi install`) provides compilers and runtimes for all 7 languages.
- Complete parity of testing commands (`pixi run test-all`) in local development and CI.
- No requirement for globally installed system packages or conflicting language version managers.

### Negative
- Initial environment solver resolution takes longer due to multi-language dependency graphs.
- Manifest files require synchronized version constants (`VERSION = "0.1.0"`).
