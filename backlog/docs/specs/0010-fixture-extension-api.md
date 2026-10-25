# Specification 0010: Fixture Extension API

## Overview

Defines the stable extension surface that lets fixture providers — first-party or third-party — add fixtures to Playtest without modifying core packages. This specification is part of the **frozen v1.0.0 core**: together with [Spec 0004](./0004-isolation-and-vcr-fixtures.md) and [Spec 0005](./0005-polyglot-1-to-1-fixture-and-bdd-parity.md) it forms the stability boundary recorded in [ADR 0014](../adr/0014-v1-stability-boundary-and-fixture-extension-model.md). Everything built on this surface after v1.0.0 — including the first-party package family in [Spec 0009](./0009-ecosystem-integration-fixtures.md) — is ecosystem: independently versioned, separately reported, never required by core.

## 1. Stability Contract

- Spec 0010 is frozen at `v1.0.0` with the core. Post-1.0 evolution of the extension surface is **SemVer-additive only**: new optional hooks with sane defaults may appear in minors; changing or removing an existing extension point requires a major release of the host package.
- The host packages report the extension API level they implement (for example in `playtest version --format json`), so providers can declare and check a compatible core version range.
- The API level covers the in-language surface (traits, plugins, module contracts); the CLI's JSON schemas evolve under their own rules ([Spec 0008 §7](./0008-cross-language-cli-and-toolchain-detection.md)).

## 2. Required Extension Semantics

A registered fixture provider must be able to, in every host language:

1. **Declare** fixtures with the scopes defined in [Spec 0001](./0001-fixture-engine-and-scopes.md), without access to core-internal APIs.
2. **Resolve requested-only**: a provider fixture is never constructed unless a test requests it (or requests a fixture that depends on it).
3. **Compose**: a provider fixture may depend on core fixtures and on other provider fixtures (`tmpdir`, `clock`, `seed`, …), receiving them through the ordinary dependency-injection contract.
4. **Participate in teardown**: provider teardown joins the shared stack and obeys strict failure-safe LIFO, including when setup or teardown itself fails.
5. **Report actionable diagnostics**: when a provider's runtime (toolkit, webview, tty, browser) is unavailable, the provider surfaces a structured reason that feeds explicit-selection failure ([Spec 0008 §6](./0008-cross-language-cli-and-toolchain-detection.md)) and optional doctor probes ([Spec 0008 §8](./0008-cross-language-cli-and-toolchain-detection.md)).
6. **Be discoverable**: registered providers and their fixtures are enumerable through the CLI (`playtest list --format json`) so tooling and agents can see what the workspace provides.

## 3. Per-Language Mapping

| Host | Registration mechanism | Dependency model |
| :--- | :--- | :--- |
| **Rust** | Provider crates expose fixtures through public traits/macros at the documented registration point | Ordinary cargo dependencies; heavyweight deps live only in the provider crate |
| **Python** | pytest plugin entry points using the Playtest fixture facade; providers are installable PyPI packages | Entry-point discovery; Playtest core never imports a provider |
| **TypeScript** | Packages extending the test runtime through the documented extension object (destructured-fixture style) | Ordinary npm dependencies; optional peers stay in the provider package |

Targets beyond these three adopt the same contract when their core parity ([Spec 0005](./0005-polyglot-1-to-1-fixture-and-bdd-parity.md)) lands; the contract does not change per target, only the mapping row.

## 4. Provider Conformance Profile

- The conformance suite (task_023) gains a provider profile verifying, for a reference provider: registration, requested-only construction, composition with core fixtures, LIFO teardown after injected failures/panics, structured unavailability diagnostics, and CLI discovery reporting.
- A package may claim "Playtest provider compatible" only by passing the profile against a declared core version range; results are reported separately from core parity (they never count toward Spec 0005 claims).
- The in-repo clap canary (task_048) proves, before the v1.0.0 freeze, that the public surface suffices: a provider must be implementable **without core-internal APIs**. Any canary need for private access is a blocking API gap.

## 5. Non-Goals

- No plugin system for the CLI itself beyond the probe registry already specified in Spec 0008 §8.
- No cross-language ABI claims: each host's surface follows its own ecosystem's versioning; only the semantics in §2 are uniform.
- No change to the core parity definition: providers and their fixtures are ecosystem capabilities under Spec 0009, always reported separately.
