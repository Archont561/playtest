# Architectural Decision Records (ADRs)

This directory documents key architectural decisions made for the Playtest ecosystem.

## Status Values

- **Proposed**: written down, not yet decided.
- **Accepted**: decided and in force.
- **Accepted (amended)**: in force, with the changes listed in the record's own Amendments section. The status names the amending record when there is one, as in "amended by 0015".
- **Under review by NNNN**: added to an accepted status when a later record conflicts with part of it and the conflict is not decided yet. The record still describes the implemented behavior.
- **Superseded by NNNN**: replaced by a later record and kept for history.

A record's Context, Decision, and Consequences are not rewritten after acceptance. Later changes are appended as an Amendments or Related Decisions section, or made by a new record.

| ADR | Title | Status | Date |
| :--- | :--- | :--- | :--- |
| [0001](./0001-two-crate-monorepo-architecture.md) | Two-Crate Monorepo Architecture | Superseded by 0015 | 2026-10-23 |
| [0002](./0002-async-fixture-trait-and-lifo-teardown.md) | Async Fixture Trait and Strict LIFO Teardown | Accepted | 2026-10-23 |
| [0003](./0003-code-first-bdd-without-feature-files.md) | Code-First BDD Without Feature Files | Accepted | 2026-10-23 |
| [0004](./0004-drop-browser-support-focus-on-backend.md) | Drop Browser Support; Focus on Pure Backend Systems | Accepted | 2026-10-23 |
| [0005](./0005-hermetic-pixi-and-nextest-toolchain.md) | Hermetic Pixi Environment and Cargo-Nextest Runner | Accepted | 2026-10-23 |
| [0006](./0006-crates-io-trusted-publishing.md) | Crates.io Trusted Publishing via GitHub Actions OIDC | Accepted (amended by 0015) | 2026-10-23 |
| [0007](./0007-polyglot-monorepo-architecture.md) | Polyglot Monorepo Architecture via Pixi Workspaces | Accepted (amended by 0015, 0016) | 2026-10-23 |
| [0008](./0008-two-phase-artifact-release-pipeline.md) | Two-Phase Artifact-Driven Release Pipeline | Accepted (amended by 0016) | 2026-10-23 |
| [0009](./0009-python-only-repo-automation.md) | Python-Only Rule for Repository Automation | Accepted | 2026-10-23 |
| [0010](./0010-milestone-driven-autorelease-lifecycle.md) | Milestone-Driven Auto-Release Lifecycle | Accepted (amended; scoped tags under review by 0015) | 2026-10-23 |
| [0011](./0011-cross-language-cli-and-compatibility-architecture.md) | Cross-Language CLI & Toolchain Compatibility Architecture | Accepted (amended by 0015, 0018) | 2026-10-23 |
| [0012](./0012-machine-consumable-and-extensible-cli-contract.md) | Machine-Consumable and Extensible CLI Contract | Accepted (parts 1–2 in force) | 2026-10-25 |
| [0013](./0013-scoped-browser-drop-and-ecosystem-integration-fixtures.md) | Scoped Browser Drop and Ecosystem Integration Fixtures | Proposed | 2026-10-25 |
| [0014](./0014-v1-stability-boundary-and-fixture-extension-model.md) | v1 Stability Boundary and Fixture Extension Model | Proposed | 2026-10-25 |
| [0015](./0015-cli-first-distribution-and-five-crate-workspace.md) | CLI-First Distribution with Embedded SDKs and a Five-Crate Workspace | Accepted (amended by 0020) | 2026-11-11 |
| [0016](./0016-cli-binary-distribution-channels.md) | Binary Distribution Channels for the CLI | Accepted | 2026-11-11 |
| [0017](./0017-committed-sdk-mirrors-and-explicit-wiring.md) | Committed SDK Mirrors and Explicit Wiring | Accepted | 2026-11-11 |
| [0018](./0018-sdk-reporters-and-report-protocol.md) | SDK Reporters and the Report Protocol | Accepted | 2026-11-11 |
| [0019](./0019-runner-neutral-test-declaration-for-pure-runners.md) | Runner-Neutral Test Declaration for Post-v1 Pure Runners | Proposed | 2026-11-11 |
| [0020](./0020-solid-brotli-compression-for-embedded-sdk-assets.md) | Solid Brotli Compression for Embedded SDK Assets | Accepted | 2026-11-11 |
