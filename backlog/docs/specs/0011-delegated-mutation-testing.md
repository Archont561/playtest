# Specification 0011: Delegated Mutation Testing

## Overview

Specifies how Playtest supports **mutation testing** — a meta-quality technique that measures a suite's fault-detection power by injecting small source changes ("mutants") and checking that the suite fails on them. Playtest **does not implement mutation engines**: it delegates to each target's native mutator and aggregates normalized results, following the same decentralized pattern as LCOV coverage in [Spec 0008 §4](./0008-cross-language-cli-and-toolchain-detection.md). Ships with the CLI in Milestone M5 (`v0.5.0`). The scope classification of this technique — against A/B testing, canary deployments, load testing, and the rest — is recorded in [caveats.md §5](../caveats.md).

## 1. Positioning

- Mutation score complements coverage: coverage shows what executed, mutation score shows what the suite would actually catch. A high score never proves correctness.
- Engines stay external and per-ecosystem — documented candidate mappings include `cargo-mutants` (Rust), `mutmut` (Python), and Stryker (TypeScript); target documentation declares the authoritative mapping, exactly as runner mappings do (Spec 0008 §1).
- Mutation presupposes deterministic suites: fixed seeds, fake clocks, and hermetic networking (Spec 0004) exist precisely so repeated engine runs are meaningful. Flaky suites produce meaningless scores; this precondition is documented with remediation guidance, not silently ignored.
- Mutation results are quality metrics like coverage; they never serve as conformance or parity evidence.

## 2. CLI Grammar

```text
playtest mutation [OPTIONS]

Options:
  --lang <TARGET>   Select one or more targets (as in Spec 0008 §3)
  --filter <PAT>    Restrict mutation to a subset via the native mutator's filter mapping
  --format <FMT>    Output: pretty (default), json, junit
  --min-score <PCT> Fail when the aggregate or per-target score drops below PCT
  --fail-fast       Stop after the first failing target
```

Selection follows [Spec 0008 §6](./0008-cross-language-cli-and-toolchain-detection.md): `playtest mutation` is by definition an explicit request, so a missing engine for a selected target exits 3 with remediation (never a green skip), and a selection matching no target exits 2. A score below `--min-score` is a test failure (exit 1).

## 3. Normalized Result Contract

Each target's engine output is reduced to a record with at least:

- `mutants_total`, `killed`, `survived`, `timeout`, `excluded` — counts.
- `score` — killed ÷ (total − excluded), with the denominator's exclusions visible.
- `engine` and `engine_version` — what actually ran.
- `survivors` — surviving mutant locations (`path`, line/span, mutator kind) so a failure names what to fix, mirroring failing-test identification in Spec 0008 §7.

Records aggregate into one document carrying `schema_version` under the Spec 0008 §7 machine contract (stdout data only, diagnostics on stderr, no prompts or color in machine modes). `--format junit` emits standard test-run XML where each target is a suite and each surviving mutant is a named failing case.

## 4. Doctor Integration

- Configured mutators are **optional tools**: `playtest doctor` reports their presence and version when a workspace declares mutation configuration, distinguishing them from required components (Spec 0008 §2).
- Consumer-registered probes (Spec 0008 §8) may add environment checks a mutator needs (compiler toolchains, sandbox permissions).

## 5. Boundaries and Non-Goals

- No mutation engine is implemented inside Playtest, in any language; wrapping, forking, or vendoring an engine is out of scope.
- Scores are per-target quality signals; they never count toward Spec 0005 parity and never block conformance claims on their own — thresholds are a consumer policy.
- Techniques outside the deterministic pre-deployment category (A/B testing, canary deployments, load/stress testing, security scanning, chaos platforms) remain out of scope per [caveats.md §5](../caveats.md); mutation is included precisely because it is deterministic and pre-deployment.
