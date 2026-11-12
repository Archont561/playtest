# ADR 0014: v1 Stability Boundary and Fixture Extension Model

## Status
Proposed

## Context

`v1.0.0` must mean one thing across all nine packages: the **stable Playtest core** — fixture engine and scopes, testing techniques (BDD, property testing, test controls), and the isolation/determinism helpers — frozen. The roadmap then splits on how ecosystem support (application fixtures for CLIs, TUIs, GUIs, browsers, desktop/mobile apps) is delivered after v1:

| | **A — in-package integration layer** | **B — extension API + separate packages** |
| :--- | :--- | :--- |
| DX | One package, fixtures found in place | Two-step install; discovery work needed |
| API risk | None (internal APIs usable) | Extension surface must be designed early |
| v1 stability story | Contract freezes, artifacts keep churning for ecosystem reasons | Core artifact **and** extension contract freeze at 1.0; after that, only ecosystem releases |
| Weight (ADR 0004) | Optional heavy deps still enter core manifests | Core manifests untouched |
| Ecosystem | First-party only, forever | First-party *and* third-party fixture packages |
| Maintenance | One repo, atomic changes, cheapest to build | Core×provider compatibility matrix, new release slots |

Two factors decide it. First, the user-facing rule *"after v1, only ecosystem support"* is only literally true under B — an in-package layer forces core minors for every ecosystem bump (Tauri, Qt, Playwright majors). Second, the host ecosystems themselves are plugin-shaped: pytest's equivalents of our Python fixtures (`pytest-qt`, `pytest-playwright`) are plugins, vitest ships `@vitest/browser` as a separate package, and Rust integrates capabilities through separate crates. The decisive cost of B is timing: **the extension API is itself a core capability**, so it must land *before* v1 — and it must be validated early enough not to freeze blind.

## Decision

1. **v1.0.0 freezes three things**: Spec 0004 (helpers), Spec 0005 (core parity), and **Spec 0010 (Fixture Extension API)**. Core package releases after 1.0 are fixes-only.
2. **The extension API ships in M6 (pre-v1)**, deliberately minimal — declare fixtures/scopes, requested-only resolution, composition, failure-safe LIFO participation, structured diagnostics, discovery — and is validated by an in-repo, dependency-free **clap canary** (task_048) built exclusively against the public surface. Canany needs for core-internal APIs block the freeze.
3. **Post-v1 fixture families ship as separate first-party packages**, hosted in this monorepo but released as independent artifacts with their own versions and provider-conformance reports: `playtest-clap`, `playtest-tui`, `playtest-tauri` (Rust), `playtest-qt`, `playtest-tk` (Python), `@archont561/playtest-browser` (npm) — the Spec 0009 family (names provisional).
4. **Third-party providers are first-class**: the same Spec 0010 conformance profile applies, so the ecosystem is not limited to what this repository ships.
5. **ADR 0004 is preserved in full**: heavyweight dependencies (Playwright, Qt, Tauri/webview runtimes, emulators) live only inside provider packages; core installs stay lean.

## Consequences

### Positive
- "After v1, only ecosystem support" becomes literally true: core artifacts and contracts freeze together; every later feature release is a provider package.
- Ecosystem churn (browser/toolkit majors) decouples from core releases; provider packages can iterate at their own cadence.
- Third parties can ship fixtures without forking; the monorepo keeps first-party providers atomic with the core they test against.
- The canary de-risks the pre-v1 API freeze with a zero-weight, in-repo consumer.

### Negative
- The API still ships designed against one small canary, not against Tauri/Qt reality; the additive-only evolution rule is the standing mitigation, and first-party packages in 1.1.x are the first real stress test.
- More artifacts to release, document, and discover: release-pipeline slots, docs-portal sections, and `playtest list` reporting must surface the package family.
- A core×provider compatibility matrix appears; providers must declare core version ranges and pass the provider profile to claim compatibility.

## Related Decisions

**2026-11-11.** Three later records touch this boundary. None of them changes the Decision above.

- [ADR 0015](./0015-cli-first-distribution-and-five-crate-workspace.md) gives every release one version across the CLI and all SDKs. How that combines with "core package releases after 1.0 are fixes-only" is an open question recorded there.
- [ADR 0018](./0018-sdk-reporters-and-report-protocol.md) adds the report protocol between the CLI and the SDKs. Whether it joins the freeze list is an open question recorded there.
- [ADR 0019](./0019-runner-neutral-test-declaration-for-pure-runners.md) (Proposed) would add a runner-adapter surface to the freeze list and change the Python registration row of Spec 0010 §3.
