# Playtest session templates

Use only the artifact that fits the moment. Fill in evidence from the current checkout; do not carry
forward a stale test count or claim a PR/merge that has not happened.

## 1. Session opening prompt

```text
We are working in Playtest. Read `AGENTS.md` and `.agents/skills/session/SKILL.md`, inspect the
current branch and working tree, and preserve any existing user changes. The last verified baseline
was <N> tests passing / <M> skipped on <date and command>, or “not recorded.”

I want to work on <task ID or bounded objective>. Relevant settled decisions: <ADRs/specs or
“none known”>. Please <specific authorized action>; ask before changing public behavior or scope.
```

## 2. Session proposal

```text
Session proposal — <date>

Checkout: <branch>; working tree <clean / existing changes noted>.
Environment: Pixi <available / unavailable>; <relevant fact>.
Scope: <requested objective or backlog-selection scope>.

Recommended task(s), in order:
1. <task ID> — <outcome, dependency status, and why it is a fit>.

Not selected: <task ID — blocker or reason>, if useful.
Decisions needed: <specific decisions that change scope or behavior, or “none”>.

Checks expected: <focused test/lint/build tasks>.
Need from you: <only the decision or authorization that is actually required>.
```

If the user already named and authorized the task, skip the proposal/wait step and proceed.

## 3. Task hand-off

```text
<task ID or objective> — <short title>

Changed: <paths and purpose>.
Evidence: <command or inspected behavior> → <result>.
Checks: <command> → <result>; <unrun check> → <reason>.
Backlog: <task status / acceptance criteria update, or none>.
Left open: <remaining work or “nothing known”>.
```

## 4. Session report

```text
Session report — <date>

Outcome: <completed on this branch / awaiting review / merged via PR #N / no implementation>.
Changed: <paths, commits if any, and concise summary>.
Checks: <commands and actual results>.
Backlog: <task IDs and statuses, only if updated>.
Not done: <unproven acceptance criteria, external dependencies, or “none known”>.
Next step: <one actionable continuation>.
```
