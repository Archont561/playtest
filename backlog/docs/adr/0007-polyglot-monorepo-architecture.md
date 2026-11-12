# ADR 0007: Polyglot Monorepo Architecture via Pixi Workspaces

## Status
Accepted. Amended by [ADR 0015](./0015-cli-first-distribution-and-five-crate-workspace.md) and [ADR 0016](./0016-cli-binary-distribution-channels.md) on 2026-11-11; see Amendments below.

## Context
While `playtest` originated as a Rust-centric test runner, unified testing ergonomics across modern systems and services require seamless interoperability across multiple programming languages. The project targets the same observable core testing capabilities across all nine language targets: Rust, Python, TypeScript, C++, C, C#, Go, Java, and R. The repository therefore needs a cohesive multi-package monorepo architecture. Managing multiple disparate language package managers without version collision or drift introduces substantial friction.

## Decision
We adopted a unified polyglot monorepo structure orchestrated by **Pixi** (`pixi.toml`):
1. **Root Pixi Workspace**: Root `pixi.toml` owns shared developer tools and the environments selected for sandbox bundles. Dependencies needed by a bundled root environment must be declared there.
2. **Subproject Manifests**: Language targets reside in their own directories (`crates/`, `python/`, `typescript/`, `cpp/`, `c/`, `csharp/`, `go/`, `java/`, `r/`) with package-scoped manifests and native package metadata.
3. **Delegation Tasks**: Root `pixi.toml` exposes aggregate tasks (`lint-all`, `test-all`, `build-all`, `package-all`, `release-all`) to orchestrate target workflows. Delegating to a subproject Pixi manifest creates/uses that manifest's environment; it is not equivalent to including those dependencies in the root bundle.

## Consequences
### Positive
- Pixi provides a reproducible orchestration layer for all nine language targets, while individual subproject environments may remain separate.
- Aggregate testing commands (`pixi run test-all`) provide one entry point in local development and CI; they must not be mistaken for proof of feature parity or offline readiness until every target workflow and dependency is verified.
- No requirement for globally installed system packages or conflicting language version managers.

### Negative
- Initial environment solver resolution takes longer due to multi-language dependency graphs.
- Manifest files require synchronized version constants (`VERSION = "0.1.0"`).

## Amendments

**2026-11-11.** The directory layout and the Pixi orchestration of this record stand. Two things change:

- The language directories are also the source of the SDK files that the CLI embeds ([ADR 0015](./0015-cli-first-distribution-and-five-crate-workspace.md)). They are no longer nine independently versioned packages: a release has one version across the CLI and every SDK.
- Registry packages carry the CLI binary, or a wrapper for it, next to the SDK ([ADR 0016](./0016-cli-binary-distribution-channels.md)).
