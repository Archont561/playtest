# Playtest Technical Documentation & Backlog

This directory contains the architecture decisions, milestone roadmaps, specifications, and operational workflows for `playtest`.

## Structure

- **[Milestones](milestones/)**: Roadmap milestones bound to explicit semantic versions (`v0.1.0` through `v1.0.0`).
- **[Architecture Decision Records (ADRs)](adr/)**: Numbered design decisions and rationale.
- **[Specifications](specs/)**: Detailed subsystem specifications, public API contracts, and lifecycle behaviors.
- **[Workflow](workflow.md)**: Development lifecycle, TDD red-green-refactor loop, and definition of done.
- **[Caveats & Non-Goals](caveats.md)**: Explicit scope boundaries (e.g. dropping browser automation).

## Milestones Summary

| Milestone | Target Version | Scope | Status |
|:--|:--|:--|:--|
| **M1** | `v0.1.0` | Core Fixture Engine, LIFO Teardown, and Declarative `test!` Macro | In Progress |
| **M2** | `v0.2.0` | Procedural Attributes (`#[prop]`, `#[skip]`, `#[skip_if]`, `#[cases]`) | Planned |
| **M3** | `v0.3.0` | Code-First Fluent BDD Scenarios (`test_scenario!`) & PBT Combination | Planned |
| **M4** | `v0.4.0` | Standard Isolation Fixtures (`tmpdir`, `snap`, `cassette`, `env`, `clock`, `network_guard`, `seed`) | Planned |
| **M5** | `v1.0.0` | Documentation, Benchmarks, Conformance Suite, Production 1.0 Release | Planned |
