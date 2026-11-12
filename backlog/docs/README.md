# Playtest Technical Documentation & Backlog

This directory contains the architecture decisions, milestone roadmaps, specifications, and operational workflows for `playtest`.

## Structure

- **[Milestones](milestones/)**: Roadmap milestones bound to explicit semantic versions (`v0.1.0` through `v1.1.0`).
- **[Architecture Decision Records (ADRs)](adr/)**: Numbered design decisions and rationale.
- **[Specifications](specs/)**: Detailed subsystem specifications, public API contracts, and lifecycle behaviors.
- **[Workflow](workflow.md)**: Development lifecycle, TDD red-green-refactor loop, and definition of done.
- **[Caveats & Non-Goals](caveats.md)**: Explicit scope boundaries (e.g. dropping browser automation).

## Milestones Summary

The authoritative table, with links to each milestone file, is [milestones/README.md](milestones/README.md).

| Milestone | Target Version | Scope | Status |
|:--|:--|:--|:--|
| **M1** | `v0.1.0` | Monorepo scaffolding and package reservations (done); five-crate workspace, the `playtest` CLI with `version`, `init`, and `sync`, and its distribution channels (open) | In Progress (reopened) |
| **M2** | `v0.2.0` | Core fixture engine, LIFO teardown, declarative `test!` macro; Python and TypeScript fixture bridges | Ready |
| **M3** | `v0.3.0` | Code-first fluent BDD scenarios and property-based testing | Backlog |
| **M4** | `v0.4.0` | Isolation fixtures, snapshots, and VCR cassettes | Backlog |
| **M5** | `v0.5.0` | Core parity across all nine targets, conformance suite, CLI orchestration (`list`, `doctor`, `test`, `mutation`), report protocol | Backlog |
| **M6** | `v1.0.0` | Stable APIs, frozen fixture extension API, coordinated `v1.0.0` release | Backlog |
| **M7** | `v1.1.0` | First-party ecosystem integration fixture packages | Backlog |
