# Specification 0009: Ecosystem Integration Fixtures (Desktop GUI & Browser)

## Overview

Specifies the post-core, per-package integration layer that goes beyond the shared contract in [Spec 0005](./0005-polyglot-1-to-1-fixture-and-bdd-parity.md): desktop GUI application fixtures for the **Python** package (Qt and tkinter) and browser fixtures for the **TypeScript** package (client-side JavaScript). These capabilities ship after the receiving package's core common capabilities are complete, as SemVer-minor additions in Milestone M7 (`v1.1.0`); [ADR 0013](../adr/0013-scoped-browser-drop-and-ecosystem-integration-fixtures.md) records the scoping relative to [ADR 0004](../adr/0004-drop-browser-support-focus-on-backend.md).

## 1. Positioning and Non-Goals

- Core stays free of heavyweight external binaries: neither Spec 0005, the conformance suite, nor the Rust target depends on browser automation or GUI toolkits, and core installs pull no browser downloads (ADR 0004 preserved for core).
- Integration fixtures are optional, requested-only capabilities: a test that never requests them never creates them, and their absence cannot affect core parity claims.
- Every integration reuses the core guarantees: dependency-injected fixtures, strict failure-safe LIFO teardown, skip/conditional-skip controls, and the Spec 0004 determinism helpers where they apply.
- Integration capabilities have their own per-target conformance checklist and are always reported separately from Spec 0005 parity (§4).
- Missing or unqualified runtimes follow [Spec 0008 §6](./0008-cross-language-cli-and-toolchain-detection.md): explicitly requested means failure with remediation, never a green skip.

## 2. Python Desktop GUI Fixtures

### 2.1 `qt_app` — Qt application host

- Provides a Qt application host supporting PyQt6 and PySide6, with automatic binding selection and a documented override.
- The `QApplication` is a process-level singleton: the fixture owns its single creation at session scope, exposes it to every test in the process, and never creates a second one. Requested-only: a test that does not request `qt_app` starts no Qt application.
- Default platform is headless (`QT_QPA_PLATFORM=offscreen`) unless the test or environment requests a visible platform; a requested run whose platform cannot start fails with remediation (Spec 0008 §6) instead of skipping green.
- Per-test top-level windows/widgets are created through fixture factories and closed in strict LIFO order, including after assertion failures.
- Event-loop pumping helpers process events and wait for conditions with timeout budgets taken from the injected `clock` fixture; where a toolkit only supports wall-clock waits, the fallback is documented and bounded.
- Teardown closes windows, drains pending events, and releases the application at its owning scope end — failure-safe in every case.
- Interaction helpers stay minimal and scriptable: locate widgets by object name/role, activate, type, and assert signal emission. A full page-object framework is out of scope.

### 2.2 `tk_app` — tkinter application host

- Provides a fresh `Tk` root per test by default, withdrawn unless the test requests visibility, destroyed in strict LIFO order on teardown (including after failures).
- Event pumping uses `after`-based scheduling with timeout budgets from the injected `clock`, mirroring the Qt helper semantics.
- Interaction and reporting shapes mirror §2.1 so the two toolkits are ergonomically interchangeable.
- tkinter ships with CPython; a host interpreter built without Tk that receives an explicit request fails with remediation instead of skipping green.

### 2.3 Shared Python Rules

- Harness logic remains pure and unit-testable without a display, following the qualification-harness pattern: pure report/setup helpers covered by display-free tests, end-to-end runs opt-in.
- The `clock` and `seed` fixtures drive GUI timing and generated interaction order; `network_guard` applies to outbound sockets opened by the application under test.
- Neither fixture becomes a dependency of core installs; both live in the integration extra of the Python package.

## 3. TypeScript Browser Integration

- A `browser` fixture launches Chromium through Playwright, which is an **optional peer dependency**: core installs of `@archont561/playtest` download no browser binaries (ADR 0004 preserved for core; ADR 0013 scopes the drop).
- Fixtures follow the destructured fixture dependency-injection shape of the TypeScript runner wrapper (task_008): `test('...', async ({ browser, context, page }) => …)` — the runner contract itself does not change.
- Isolation defaults to a fresh browser context per test; contexts, pages, and the shared browser close in strict LIFO order after test failures. Browser/session scope reuse is documented when opted into.
- Client-side JavaScript runs in the page; fixtures expose navigation, evaluation, and console/network capture with deterministic waiting (no wall-clock sleeps outside Playwright's own auto-waiting).
- Network interplay is documented and mapped: Playwright request interception corresponds to `cassette` record/replay phases and `network_guard` default-deny semantics, with conformance coverage where the mapping is total and documented gaps where it is not.
- A missing or unlaunchable browser under explicit selection fails with remediation, surfaced through Spec 0008 §6/§8 (a doctor probe reports the installed browser and channel).

## 4. Conformance, Tiers, and Reporting

- The CLI declares additional test tiers beyond `unit` and `smoke`: `gui` and `browser`, each mapped per target and visible in `playtest list` (Spec 0008 §6).
- Integration conformance cases verify: requested-only creation, failure-safe LIFO teardown after injected failures, fail-loud behavior for unqualified hosts under explicit selection, and separateness from core parity reports.
- A target may hold full Spec 0005 core parity while having no integration coverage; the reverse must never be implied — integration results never count toward core parity.
- Reports identify the tier and integration kind so CI can schedule heavy tiers independently of the core suite.
