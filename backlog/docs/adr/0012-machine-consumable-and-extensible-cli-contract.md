# ADR 0012: Machine-Consumable and Extensible CLI Contract

## Status
Accepted (2026-11-11). Parts 1 and 2 are in force; parts 3 to 5 are accepted as the
target contract and land with the `playtest test` and `playtest doctor` commands in
M5. See the Amendments section.

## Context

The `playtest` CLI specified in [Spec 0008](../specs/0008-cross-language-cli-and-toolchain-detection.md) has three consumer classes: humans reading `pretty` output, CI ingesting JUnit XML, and automated consumers — scripts and AI agents — that parse JSON and branch on exit status. Today the specification fixes the command grammar and the `--format` options but leaves exit-status meanings, schema stability, empty-selection behavior, and the missing-toolchain case implicit. Implicit defaults are the worst case for automated consumers: a selected target that quietly skips and exits 0 is indistinguishable from success, and an agent cannot tell "your code failed" from "you called this wrong" from "the host is unqualified".

Two concrete pressures make this concrete:

1. **Consumer qualification harnesses.** Repositories that embed Playtest-shaped testing in heavier environments run smoke/qualification harnesses the CLI cannot know about — for example [Archont561/qgis-sdk](https://github.com/Archont561/qgis-sdk), which launches a real desktop application headless, gates suites behind environment flags, and records per-harness JSON reports as release evidence (including during CI outages). Such consumers need explicit tier selection with fail-instead-of-skip semantics, custom compatibility probes (install prefix, bindings, display server), and one normalized evidence report across pytest and a second language target.
2. **AI-agent consumption.** Agents driving test loops need a stable machine contract: documented exit codes, versioned JSON, non-interactive stream discipline, and guidance reachable from the tool itself rather than only from prose documentation.

## Decision

We make the CLI a programmable interface by contract, in five parts:

1. **Exit-status contract** (Spec 0008 §7): `0` pass, `1` test failure, `2` usage/selection error, `3` environment failure. Environment problems never masquerade as test outcomes, and usage errors never masquerade as environment problems.
2. **Versioned machine output** (Spec 0008 §7): every JSON document carries `schema_version`; stdout carries data only; machine modes never prompt and never emit ANSI color; failing property tests carry their seed/replay command.
3. **Explicit selection fails loud** (Spec 0008 §6): `--lang`/`--tier`/`--filter` are explicit requests; a selected-but-unavailable target exits `3` with doctor-style remediation instead of skipping green. The `--tier` grammar (at least `unit` and `smoke`) maps each tier to the target's native marker/filter mechanism, and the rule holds under direct native invocation through Playtest-provided runner integrations. `--skip-unavailable` is the explicit opt-out.
4. **Extensible `doctor` probes** (Spec 0008 §8): consumers register workspace-local compatibility probes with a versioned record shape (`id`, `observed`, `ok`, `required`, `remediation`); they fold into the unified doctor report and exit rules but can never claim framework conformance.
5. **Shipped consumer agent skill**: a `playtest` skill documents grammar, exit codes, tier selection, and probe registration for consumer repositories, complementing this repository's `AGENTS.md`, which remains the authority for contributors working *on* Playtest.

## Consequences

### Positive
- One command plus one JSON document replaces per-repo conventions for polyglot smoke runs, and evidence becomes machine-checkable instead of copy-pasted.
- Agents and CI branch on stable exit codes and a versioned schema; fail-loud selection removes the silent-green class of orchestration bugs.
- Consumer-specific qualification (desktop prefixes, bindings, display servers) plugs into `doctor` without forking the CLI.
- The shipped skill carries the contract to consumer repositories, where `AGENTS.md` does not reach.

### Negative
- Exit statuses and `schema_version` are forever contracts; loosening them later is a breaking change even where no code exists yet.
- The probe registry is one more versioned surface, and tier mappings are uneven across nine native runners — unmapped tiers must surface as errors in `playtest list` rather than be silently dropped.
- Framework-side parity for fail-loud tiers adds a required behavior to every target's runner integration (Spec 0005 §4 cross-reference), growing the conformance matrix.

## Related Decisions

**2026-11-11.** [ADR 0015](./0015-cli-first-distribution-and-five-crate-workspace.md), [ADR 0017](./0017-committed-sdk-mirrors-and-explicit-wiring.md), and [ADR 0018](./0018-sdk-reporters-and-report-protocol.md) use exit status `3` for three more environment failures: a `playtest.toml` version pin that the running binary does not match, an SDK whose version or protocol differs from the CLI's, and SDK mirrors that `playtest sync --check` finds out of date. Spec 0008 §7 lists them. The contract in this record is otherwise unchanged.

## Amendments

**2026-11-11 — accepted ahead of full implementation.** This record was proposed on
2026-10-25 while the CLI did not yet exist. It is accepted now because `v0.1.0` ships the
`playtest` binary, which already implements two of its five parts and reuses their exit
statuses in `init` and `sync`. Shipping a release on top of a contract that is still
marked "not yet decided" would freeze the behaviour in published artifacts before the
record that defines it was ever accepted.

Implementation status at acceptance, so the record does not overstate what ships in
`v0.1.0`:

1. **Exit-status contract — implemented.** `ExitStatus` in `playtest-core` defines `0`
   pass, `1` test failure, `2` usage error, and `3` environment failure, and the binary
   returns them.
2. **Versioned machine output — implemented.** Every JSON document carries
   `schema_version`, and machine modes do not prompt or colour.
3. **Explicit selection fails loud — accepted, not implemented.** `--tier` and
   `--skip-unavailable` arrive with `playtest test` (`task_036`). The grammar is fixed
   here so the flag names are not invented later.
4. **Extensible `doctor` probes — accepted, not implemented.** `playtest doctor` is
   `task_025` and `task_037`; no `doctor` subcommand exists in `v0.1.0`.
5. **Shipped consumer agent skill — accepted, not implemented.** `v0.1.0` ships the
   repository's contributor skills, not a consumer-facing `playtest` skill.

Parts 3 to 5 are therefore commitments about the shape of the M5 commands, not
descriptions of `v0.1.0`. Nothing in parts 1 and 2 may be changed without a superseding
record, because published binaries already emit them.
