---
name: arena-agent
description: >-
  Repository-specific operating guide for an AI coding agent working in Playtest through Arena.
  Use for repo orientation, task execution, tool choice, branch and change safety, verification, and
  user-facing hand-off. Defer to AGENTS.md for binding project rules and use specialized skills for
  sessions, audits, tests, and refactoring.
---

# Arena agent guide for Playtest

This skill adapts general coding-agent work to this repository. It does not replace root `AGENTS.md`,
which is the source of truth for architecture, commands, automation, conventions, and releases.
Read it before changing files.

## Work from the user's request

- Identify the requested outcome and deliver it directly. Avoid broad rewrites, unrelated cleanup,
  or extra process steps that do not help complete the request.
- Ask a focused question only when a missing choice materially affects behavior, scope, data, or the
  deliverable. Otherwise state a reasonable assumption and proceed.
- For a task that crosses code, tests, docs, and packaging, inspect all relevant layers; keep the
  change set as small as the requested outcome allows.
- Clearly separate completed work, unverified claims, blockers, and suggested follow-ups in the
  final response.

## Repository boundaries

- Follow `AGENTS.md` and nested agent instructions. Use the target package's manifest and documented
  test entry points in this polyglot repository; do not assume a Rust-only workflow.
- Repository automation must obey the Python-only rule in `AGENTS.md`. Invoke repository tools
  through Pixi tasks. The documented `scripts/restore.sh` exception remains the only allowed shell
  helper exception.
- Preserve public contracts and cross-language parity requirements. Before changing a public API,
  wire format, release path, or documented behavior, consult the relevant specification and ADR.
- Keep generated, vendored, build, coverage, and package output out of Git unless a tracked output
  is explicitly required by the task.
- Use only the current working branch assigned to the session. Do not switch or rename branches.
  Preserve user changes; inspect `git status` and diffs before editing and before concluding.
- Never expose, request, or store credentials. Do not initiate commits, pushes, PRs, merges,
  releases, or workflow dispatches unless the user explicitly requests or authorizes them.

## Specialized workflows

- At session start, task selection, or hand-off: `.agents/skills/session/SKILL.md`.
- For a requested code-quality audit: `.agents/skills/audit/SKILL.md` (read-only except for
  validated backlog follow-ups, unless report-only was requested).
- For new behavior or bug fixes: `.agents/skills/tdd/SKILL.md`.
- For behavior-preserving cleanup after tests pass: `.agents/skills/refactor/SKILL.md`.
- For ambiguous requirements or conflicting contracts: `.agents/skills/grill-me/SKILL.md`.
- For design-quality principles: `.agents/skills/clean-code-principles/SKILL.md` and only the
  implemented rule files relevant to the evidence.

## Verification and delivery

1. Inspect the relevant source, public interfaces, tests, docs, task state, and current diff.
2. Run the narrowest useful Pixi task(s), followed by applicable broader gates. Common root tasks
   are listed in `pixi.toml`; examples include `pixi run test`, `pixi run test-all`,
   `pixi run lint`, `pixi run lint-all`, `pixi run fmt-check`, and `pixi run actionlint`.
3. If a command cannot run, give the actual reason and do not describe it as passing. Do not add
   dependencies or generated artifacts solely to make a check appear successful.
4. Report the files changed and what was verified. Include exact follow-up action when something
   remains blocked or needs a user decision.
