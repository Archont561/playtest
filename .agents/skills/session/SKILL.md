---
name: session
description: >-
  Coordinate a focused Playtest work session from repository orientation through task selection,
  implementation, verification, and hand-off. Use at the beginning of a session, when asked what to
  work on or to pick up backlog work, and when asked for a task/session summary or next-step hand-off.
  Adapt the procedure to the user's explicit request: do not pause for a proposal when the user has
  already authorized a concrete task.
---

# Playtest session lifecycle

Use this skill to keep a work session reproducible and scoped. Repository-specific rules in the
root `AGENTS.md` are authoritative; this skill organizes them into a practical flow. Do not assume
a particular working branch, test baseline, or remote state: inspect and report what is true in the
current checkout.

## 1. Orient to the checkout

1. Read `AGENTS.md` and any more-local `AGENTS.md` files for the paths in scope. For implementation,
   follow `.agents/skills/tdd/SKILL.md` before `.agents/skills/refactor/SKILL.md`; for planning,
   use `backlog/docs/workflow.md` and the relevant ADR/spec.
2. Inspect `git status --short --branch`, the recent commits, and relevant diffs before editing.
   Preserve existing user changes. Do not switch or rename the active branch; use the branch already
   assigned to this work session.
3. Check that Pixi is available and use Pixi tasks for repository tools, as required by `AGENTS.md`.
   If the environment is unavailable, report the precise blocker and avoid claiming verification.
   `scripts/restore.sh` is the one documented shell-script exception; follow its repository
   instructions when a restore is actually needed.
4. If the task or next-work choice depends on current remote state, inspect it before making a
   recommendation. Do not fetch, push, open a PR, merge, tag, publish, or dispatch workflows unless
   needed and authorized for the user's request.

## 2. Choose a bounded task

- For an explicitly requested task, restate the intended outcome briefly and start; do not add an
  unnecessary approval round.
- If asked to choose work, read `backlog/tasks.yaml`, identify unfinished tasks and dependencies,
  and check `backlog/docs/milestones/`, `backlog/docs/specs/`, and `backlog/docs/adr/` as needed.
  Recommend a bounded, unblocked task with a reason. Confirm ambiguity that changes scope, public
  behavior, or user priorities before implementing.
- Backlog statuses are `backlog`, `ready`, `in_progress`, and `done`. Every task must retain its
  `milestone` and `semver` fields. Update the YAML task list carefully; do not fabricate a new
  milestone or silently change acceptance criteria.
- Limit changes to the selected task. Record out-of-scope observations as follow-up suggestions or
  tasks only when the user requests that, or when a relevant audit has validated them.

## 3. Implement and verify

- Follow `AGENTS.md` for the strict Python-only repository automation rule, Pixi execution, allowed
  Conventional Commit types/scopes, and project-specific test and release requirements.
- For new behavior or bug fixes, use the test-first workflow in `.agents/skills/tdd/SKILL.md`.
  Keep tests at the appropriate target's established test seam and follow the contract and parity
  constraints in `AGENTS.md` and the relevant specs. Apply `.agents/skills/refactor/SKILL.md`
  separately after behavior tests pass; do not mix unrelated refactoring into feature work.
- Use the narrowest relevant checks first, then the appropriate package or repository gates. Common
  root tasks include `pixi run test`, `pixi run test-all`, `pixi run lint`, `pixi run lint-all`,
  `pixi run fmt-check`, `pixi run actionlint`, `pixi run detect-release`, and
  `pixi run package-all`. Check `pixi.toml` for the precise available task and scope. Don't run a
  costly gate unrelated to a documentation-only change unless `AGENTS.md` or the user requires it.
- Report commands and outcomes accurately, including skipped checks and environmental blockers.
  Never mark an acceptance criterion proven without evidence.
- Do not commit, push, publish, merge, or trigger releases unless the user asks or clearly
  authorizes that action. Before a commit, follow `AGENTS.md`'s commit-message validation and
  repository hooks.

## 4. Close a task or hand off

At a task boundary, summarize what changed, evidence, checks, and remaining work. Update the
corresponding task status and acceptance criteria only when the repository's documented workflow
and actual evidence support it. Do not mark unfinished work `done`.

When ending a broader session, include a concise summary and an actionable next step. If work is
waiting on user choice, external access, CI, release, or a native-platform check, state exactly what
is needed and leave it open. A post-merge report is appropriate only after a merge actually
happened; do not imply that branch work has landed on `main`.

## Session artifacts

Use the templates in [`standup-template.md`](standup-template.md) when a structured proposal,
task hand-off, or session report would help. Keep the final response proportionate to the request;
templates are aids, not a reason to manufacture process artifacts for a small, directly requested
change.
