---
name: audit
description: >-
  Run an evidence-based code-quality audit of requested files, diffs, packages, language targets,
  or the Playtest repository. Use when asked to audit, assess, scan, or review maintainability,
  architecture, readability, duplication, complexity, code smells, or refactor opportunities.
  Keep audits read-only, cite concrete evidence, and track validated actionable follow-ups in
  backlog/tasks.yaml unless the user requests report-only output. Distinguish behavior-preserving
  refactors from bugs, missing features, and contract changes. Pause for clarification when intent
  or an explicit contract is unclear. This is not a security or compliance audit.
---

# Code-quality audit

An audit identifies and prioritizes maintainability risks; it does not implement production fixes.
The deliverable is an evidence-backed report and, by default, a focused backlog task for each
confirmed, actionable follow-up.

## 1. Establish scope

- Follow the target the user names: files, diff, crate, language target, or repository. If the
  request is just “audit this” and the target is not clear from context, ask one focused scope
  question. State the scope and exclusions in the report.
- Read the root `AGENTS.md`, relevant local `AGENTS.md` files, applicable ADRs/specifications, and
  the nearest tests and documentation before judging project-specific design.
- Inspect `git status --short --branch` and the relevant diff. Treat pre-existing edits as user
  work; do not overwrite, stage, or misattribute them.
- Keep the audit read-only: no production changes, formatting, or opportunistic fixes. Honor
  explicit requests to exclude paths or to report only.
- Exclude generated, vendored, lock, build, and unrelated language-target areas unless in scope.

## 2. Apply suitable criteria

Use repository conventions and documented contracts as controlling context. For design findings,
consult `.agents/skills/clean-code-principles/SKILL.md` and read only the implemented rule files
that apply. Use `.agents/skills/refactor/SKILL.md` to distinguish a behavior-preserving cleanup
from a rewrite or behavior change. A principle is a lens, not evidence: line count, a hypothetical
pattern, or personal style preference alone is not an actionable finding.

Inspect the implementation alongside its callers, public API, tests, and relevant docs/specs/ADRs.
In this polyglot repository, check whether the same contract is implemented or specified in other
targets before treating an apparent difference as a defect. Do not assume every language has
identical implementation details; use the published parity requirements and target-specific
support as the reference.

Use relevant existing linters or tests only when they help verify a candidate. Do not add tools or
run a full-repository gate merely to make an audit look comprehensive. Follow `AGENTS.md`: execute
repository tooling through Pixi (root or target manifest). Record exact commands, results, and
environmental blockers; never imply a check ran when it did not.

## 3. Verify each finding

Retain a finding only when the inspected evidence shows a concrete maintenance cost or risk. For
each finding:

1. Cite an exact `path:line` (or exact tool output) and describe the observable issue.
2. Explain the practical impact in this repository; cite the applicable implemented rule where
   useful, but do not use a rule label as a substitute for evidence.
3. Check tests, README/API documentation, `backlog/docs/specs/`, `backlog/docs/adr/`, release
   behavior, and cross-language parity expectations for relevant contracts.
4. Recommend the smallest behavior-preserving change. Mark hypotheses and optional improvements as
   such, rather than reporting them as confirmed defects.
5. Classify the work correctly: refactor (same behavior), bug (behavior violates an established
   expectation), enhancement (missing capability), or contract change (approved intentional
   behavior/API change).

If a candidate depends on unclear intent, or implementation evidence conflicts with tests, docs,
specifications, ADRs, public API, or user constraints, stop and ask a focused clarification using
`.agents/skills/grill-me/SKILL.md` before deciding scope or writing a task. Do not encode one side of
an unresolved conflict as an acceptance criterion.

## 4. Track actionable follow-up

Unless the user explicitly asks for report-only output, create or update a backlog task for each
confirmed, actionable follow-up once scope and expected behavior are clear. Do not ask for another
permission prompt merely to record a task after the user requested an audit.

- Search `backlog/tasks.yaml` first and update/reuse an existing task where appropriate. Avoid
  duplicates; clarify ownership if existing tasks overlap or conflict.
- Preserve the file's schema and conventions: unique `task_NNN` ID, valid status (`backlog`,
  `ready`, `in_progress`, `done`), priority, milestone, semver, labels, repo target,
  acceptance criteria, and dependencies. Every task must include milestone and semver in line with
  the repository's backlog directives in `AGENTS.md`.
- Follow `backlog/docs/workflow.md` and the task conventions in `backlog/tasks.yaml`. The task
  should state the finding/evidence, bounded implementation scope, intended behavior to preserve
  or establish, non-goals where useful, and verifiable acceptance criteria.
- For refactor implementation work, consult `.agents/skills/tdd/SKILL.md`; tests must verify
  behavior rather than implementation details. Keep unrelated behavior changes separate.
- Leave follow-up tasks open. An audit does not implement a fix or mark the work complete.
- If the task file cannot be updated safely (for example, milestone/semver mapping is unclear or
  the task list has conflicting entries), report the blocker and provide a ready-to-review draft;
  do not silently invent planning decisions.

If no actionable issue is confirmed, create no task and say so.

## 5. Report

Use this compact structure, omitting empty sections:

```markdown
## Audit
**Scope:** ...
**Summary:** ...

### Findings
- **[severity] [classification] [rule-id, if applicable]** `path:line` — evidence, impact, and
  smallest recommended change.

### Follow-up tasks
- `task_NNN` — title; or explain why no task was needed / why creation was blocked.

### Checks
- `pixi run ...` — result, or exact reason it could not run.

### Scope limits
- Paths, targets, or concerns not examined.
```

Use severity proportionately. Separate confirmed findings from questions and suggestions, and make
clear that this skill addresses code quality rather than security, compliance, or product validation.
