# ADR 0019: Runner-Neutral Test Declaration for Post-v1 Pure Runners

## Status
Proposed (2026-11-11). Would amend [ADR 0014](./0014-v1-stability-boundary-and-fixture-extension-model.md) and [Spec 0010](../specs/0010-fixture-extension-api.md). Awaiting a maintainer decision.

## Context

Every target integrates with a third-party or toolchain runner today: pytest, Bun, nextest, and the others in [Spec 0005 §2](../specs/0005-polyglot-1-to-1-fixture-and-bdd-parity.md). A later option is a *pure runner*: the runner that ships with the language itself, such as Python's `unittest`, Node's `node:test`, `cargo test`, or Go's `testing`, with no extra framework to install.

[ADR 0014](./0014-v1-stability-boundary-and-fixture-extension-model.md) freezes the core at `v1.0.0` and allows only fixes afterwards. A pure runner can therefore be added after v1 only if nothing frozen at v1 assumes a particular runner. Two things assume one today:

- Tests are declared with the runner's own constructs. A test written as a pytest function does not run under `unittest`.
- [Spec 0010 §3](../specs/0010-fixture-extension-api.md) registers Python fixture providers through pytest plugin entry points.

A prototype on 2026-11-11, outside this repository, ran one Python test file under both pytest and `unittest` and one TypeScript file under both Bun and `node:test`. In both cases the tests and fixtures were declared through Playtest functions. The two Python runs wrote the same report-protocol records ([ADR 0018](./0018-sdk-reporters-and-report-protocol.md)), apart from the runner name in `hello`.

## Decision (Proposed)

To keep pure runners possible after v1, `v1.0.0` would do three more things:

1. **Tests are declared through Playtest's own API** in every SDK. Runner-specific decorators and markers are not part of the public contract.
2. **A runner-adapter surface is frozen with the core.** An adapter connects the Playtest declarations to one runner. The surface is validated before the freeze by an adapter for Python's `unittest`, built only on the public surface, in the same way the clap canary validates Spec 0010.
3. **Provider registration becomes runner-neutral.** Python providers register through a Playtest entry-point group instead of pytest's, and Spec 0010 §3 changes accordingly.

## Consequences

### If accepted
- Pure runners become additive work after v1, within ADR 0014's rules.
- The surface frozen at v1 grows by the adapter contract, and M6 gains a second canary (`task_065`).
- The Python row of Spec 0010 §3 and the registration criterion of `task_047` change before the freeze.

### If rejected
- Nothing changes before v1.
- A pure runner added after v1 needs a new major version of the affected SDKs.
