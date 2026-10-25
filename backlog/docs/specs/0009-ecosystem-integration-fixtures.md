# Specification 0009: Ecosystem Integration Fixtures (CLI, TUI, GUI & Browser)

## Overview

Specifies the first-party ecosystem fixture packages that go beyond the shared contract in [Spec 0005](./0005-polyglot-1-to-1-fixture-and-bdd-parity.md): application fixtures for the **Rust** package family (clap CLIs, ratatui TUIs, Tauri desktop/mobile apps), desktop GUI application fixtures for **Python** (Qt and tkinter), and browser fixtures for **TypeScript** (client-side JavaScript). Every fixture family is a **separate artifact** built exclusively on the frozen Fixture Extension API ([Spec 0010](./0010-fixture-extension-api.md)), versioned independently of the core packages, and shipped only after `v1.0.0` as part of Milestone M7 (`v1.1.0`). [ADR 0013](../adr/0013-scoped-browser-drop-and-ecosystem-integration-fixtures.md) records the browser/webview scoping relative to [ADR 0004](../adr/0004-drop-browser-support-focus-on-backend.md); [ADR 0014](../adr/0014-v1-stability-boundary-and-fixture-extension-model.md) records the stability boundary and the extension model.

### Package family (names provisional)

| Fixture family | Host target | Artifact |
| :--- | :--- | :--- |
| `cli_app` (clap) | Rust | `playtest-clap` (crate) |
| `tui_app` (ratatui) | Rust | `playtest-tui` (crate) |
| `tauri_app` (Tauri) | Rust | `playtest-tauri` (crate) |
| `qt_app` (PyQt/PySide) | Python | `playtest-qt` (PyPI) |
| `tk_app` (tkinter) | Python | `playtest-tk` (PyPI) |
| `browser` (Playwright) | TypeScript | `@archont561/playtest-browser` (npm) |

## 1. Positioning and Non-Goals

- Core stays free of heavyweight external binaries: neither Spec 0005, the conformance suite, nor the Rust target depends on browser automation or GUI toolkits, and core installs pull no browser downloads (ADR 0004 preserved for core; ADR 0014 decision 5).
- Integration fixtures are optional, requested-only capabilities: a test that never requests them never creates them, and their absence cannot affect core parity claims.
- Every integration reuses the core guarantees **through the Spec 0010 extension surface only** — provider packages never touch core-internal APIs, enforced by the pre-v1 canary (task_048).
- Integration capabilities have their own per-target conformance checklist (Spec 0010 §4 provider profile) and are always reported separately from Spec 0005 parity (§5).
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
- Neither fixture becomes a dependency of core installs: both ship as separate packages (`playtest-qt`, `playtest-tk`) built on the Spec 0010 entry-point mapping, depending only on the core Playtest package and their toolkits.

## 3. TypeScript Browser Integration

- A `browser` fixture launches Chromium through Playwright, which is an **optional peer dependency**: core installs of `@archont561/playtest` download no browser binaries (ADR 0004 preserved for core; ADR 0013 scopes the drop).
- Fixtures follow the destructured fixture dependency-injection shape of the TypeScript runner wrapper (task_008): `test('...', async ({ browser, context, page }) => …)` — the runner contract itself does not change.
- Isolation defaults to a fresh browser context per test; contexts, pages, and the shared browser close in strict LIFO order after test failures. Browser/session scope reuse is documented when opted into.
- Client-side JavaScript runs in the page; fixtures expose navigation, evaluation, and console/network capture with deterministic waiting (no wall-clock sleeps outside Playwright's own auto-waiting).
- Network interplay is documented and mapped: Playwright request interception corresponds to `cassette` record/replay phases and `network_guard` default-deny semantics, with conformance coverage where the mapping is total and documented gaps where it is not.
- A missing or unlaunchable browser under explicit selection fails with remediation, surfaced through Spec 0008 §6/§8 (a doctor probe reports the installed browser and channel).

## 4. Rust Application Fixtures

### 4.1 `cli_app` — CLI application fixtures (clap)

- A test drives the application's `clap::CommandFactory` definition in-process: per-test argument vectors, dispatch through the application's own parsing, and captured stdout/stderr (in-memory writers) plus a structured exit status — no shell and no global process state.
- Command handlers receive fixtures through the same dependency-injection contract as `test!` tests, so CLI logic composes with the ordinary core fixtures.
- Help, usage, and error text are golden-assertable through the `snap` fixture, with documented normalization for clap version and terminal width.
- An optional spawned-binary mode runs the real executable for end-to-end cases: spawned children are killed and reaped in failure-safe LIFO teardown, and an explicitly requested missing binary fails with remediation (Spec 0008 §6) instead of skipping green.
- This fixture family doubles as the extension-API canary (task_048): the in-repo M6 validation build, written against the public Spec 0010 surface only, graduates into the published `playtest-clap` package after `v1.0.0`.

### 4.2 `tui_app` — terminal UI fixtures (ratatui)

- The default backend is ratatui's headless `TestBackend`: no tty is required, and the terminal buffer is asserted directly or through `snap`. An opt-in real-terminal mode exists for end-to-end checks.
- Key, mouse, and paste input are injected as event objects through fixture helpers; the application's draw/tick loop is stepped with the injected `clock` — no wall-clock sleeps in the deterministic path.
- Application state and terminal buffers are per-test and dropped failure-safe in strict LIFO order, including after panics in event handling.
- An explicitly requested real-terminal run on a host without a usable tty fails with remediation instead of skipping green.

### 4.3 `tauri_app` — desktop and mobile application fixtures (Tauri)

- Two modes: an always-runnable in-process host (Tauri's test/mock builder) covering commands, managed state, and plugins — the default — and an opt-in real-host mode that launches the application with its webview on desktop, or against a mobile emulator/device session.
- The fixture owns the app handle and windows per test; windows and application state close in strict LIFO order even after panics. The `clock` and `seed` fixtures drive application timing, and `network_guard` applies to application-originated traffic.
- Real-host mode follows the qualification-harness pattern: throwaway profile/workspace, probed environment, machine-readable report, and fail-loud selection (Spec 0008 §6).
- Webview runtimes (WebKitGTK, WebView2, WKWebView) and mobile emulators are heavyweight, opt-in environment weight — never pulled by core installs — and their availability is reported by doctor probes (Spec 0008 §8). Tauri's embedded webview falls under ADR 0013's scoping of ADR 0004: webview weight exists only inside opt-in integrations, never in core.

## 5. Conformance, Tiers, and Reporting

- Each fixture family ships as its own artifact with independent versioning, release slots in the two-phase pipeline, and a provider-conformance report (Spec 0010 §4); core package releases remain core-only (ADR 0014).
- The CLI declares additional test tiers beyond `unit` and `smoke`: `gui`, `browser`, and `mobile`, each mapped per target and visible in `playtest list` (Spec 0008 §6). CLI and TUI fixtures (§§4.1–4.2) require no display or external binaries and run in the ordinary headless tiers.
- Integration conformance cases verify: requested-only creation, failure-safe LIFO teardown after injected failures, fail-loud behavior for unqualified hosts under explicit selection, and separateness from core parity reports.
- A target may hold full Spec 0005 core parity while having no integration coverage; the reverse must never be implied — integration results never count toward core parity.
- Reports identify the tier and integration kind so CI can schedule heavy tiers independently of the core suite.
