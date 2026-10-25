# ADR 0013: Scoped Browser Drop and Ecosystem Integration Fixtures

## Status
Proposed

## Context

[ADR 0004](./0004-drop-browser-support-focus-on-backend.md) dropped browser automation "completely from `playtest` core", motivated by Rust's backend-dominated use cases and the weight of Chromium binaries on CI. Two product needs now cut across that decision:

1. **Client-side JavaScript** in the TypeScript target needs real browser integration to test frontend logic — the TypeScript equivalent of what Playwright itself provides for JS ecosystems.
2. **Desktop Python** applications need GUI application fixtures (Qt, tkinter) so plugin and desktop-app authors get the same fixture/LIFO/BDD ergonomics core users have. Qualification-harness experience (headless `offscreen` platforms, throwaway profiles, failure-safe process teardown) shows the shape these fixtures must take.

Sequencing matters: both integrations should ship **after** each receiving package's core common capabilities (Spec 0005 parity, tasks `task_033`/`task_034`) are complete, so the integration layer never delays or destabilizes core parity.

## Decision

1. **Reaffirm ADR 0004 for core.** Neither Spec 0005, the shared conformance suite, nor the Rust target depends on browser automation or GUI toolkits, and core installs pull no browser binaries.
2. **Scope the drop: "from core" ≠ "never in a target".** Browser integration ships as an optional, opt-in TypeScript-target capability — Playwright as an optional peer dependency — specified in [Spec 0009 §3](../specs/0009-ecosystem-integration-fixtures.md).
3. **Python ships desktop GUI fixtures for both Qt and tkinter** (Spec 0009 §2): host-language toolkits with no new heavyweight binaries — tkinter ships with CPython, Qt is already a normal dependency of the applications being tested. Qt (PyQt6/PySide6, offscreen-capable) is the P0 fixture; tkinter is P1.
4. **Post-core sequencing as M7 (`v1.1.0`).** Integration fixtures depend on the Spec 0005 parity tasks for their package and ship as SemVer-minor work after the stable `v1.0.0` release, tracked in a dedicated milestone.
5. **Honest selection and reporting.** Missing runtimes follow Spec 0008 §6 selection semantics (explicitly requested ⇒ fail with remediation, never a green skip), integration capabilities get `gui`/`browser` tiers plus doctor probes, and their conformance results are reported separately from core parity claims.

## Consequences

### Positive
- Client-side JavaScript and desktop Python become testable with the same fixture, teardown, and scenario ergonomics as the rest of the framework.
- ADR 0004's CI-weight argument survives intact for Rust and core installs; browser weight is borne only by TypeScript users who opt in.
- Fail-loud selection, tiers, and doctor probes (Spec 0008 §§6–8) give heavy runtimes an honest, automatable contract.

### Negative
- Two new integration surfaces to maintain, each with a version matrix (PyQt6/PySide6, Qt platforms; Playwright/Chromium channels).
- ADR 0004's "completely" wording is corrected to "from core"; future integrations will cite this precedent, so the boundary needs active policing.
- GUI and browser tiers add CI cost and scheduling complexity; heavy tiers must stay schedulable independently of the core suite.
