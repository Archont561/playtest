# Session lifecycle templates

Four artifacts, one per phase, in the order a session produces them: the **opening prompt** that
starts it, the **standup** that proposes it, the **hand-off** that ends each task inside it, and
the **report** that closes it once the work has landed. Fill them in literally — same headings,
same order. They are the shape a reviewer expects, and keeping them out of `SKILL.md` means
editing a template never touches the procedure.

The loop closes on itself: the report's last block **is** the next session's opening prompt. A
session that ends without writing one has handed the next session a reconstruction job.

## 1. Session opening prompt (written at close, pasted back at the next start)

```
Install the toolchain and baseline the suite (`npm i -g bun@1.4.2`, `bun install
--frozen-lockfile`, then `bun run lint && bun run typecheck && bun test` — expect <N> passing /
<M> failing across <P> packages), then read the Backlog: `bunx backlog status` shows <X>
in_progress and <Y> ready.

<The one or two environment facts that would otherwise waste the first ten minutes: whether the
lockfile still matches the manifests, whether anything new needs installing before the tree
builds, which optional dependencies are still absent.>

<Optional, when the session's first move depends on something only the remote knows: the one
command to run, and what each outcome means. "First, one look at the repo state: `git tag -l`.
If v0.1.0 exists, task_005's publish criteria are provable from the release — read it, verify,
and close the task. If it does not exist, the release is still the open work.">

I want to take task_<n> this session — <one line on the shape, plus every decision already
made, spelled out, so the agent does not re-open settled ground>. In slices: <the locally
provable ones> first, <the ones needing a push, a publish or an absent dependency> last, which
needs <the sanction you are reserving>.

Propose the slice and stop. House rules are in `.backlog/docs/workflow.md`, the session
procedure and its templates are in `.agents/skills/session/`.
```

What makes this prompt work, and what makes it fail:

- **Name the expected test count.** It is the cheapest possible check that the install produced
  the tree the last session left; a mismatch is the first thing worth saying out loud.
- **State the sanction boundary.** Which slices may be pushed or published, and which wait for a
  click. An agent that has to guess will either stall or push something you did not want pushed.
- **Carry the decisions forward, not the deliberation.** "Method A, already decided" saves a
  round trip; "we should decide between A and B" costs one.
- **Point at the task by id**, not at "the backlog" — `bunx backlog task show task_<n>` is one
  command, and the description carries the gap the last session left.

## 2. Session standup (end of startup — then stop and wait)

```
Session proposal — <date>

Environment: bun <version> installed, `bun install --frozen-lockfile` clean.
Baseline: lint clean, typecheck clean, <N> tests passing / <M> failing across <P> packages.

Backlog: <A> done, <B> in_progress, <C> ready. Candidates, in recommended order:
1. task_<n> (<priority>, <status>) — <one line: what it delivers and why now>
2. task_<m> …
   …
Not this session: task_<k> (blocked by task_<j>); task_<l> (deferred — <one-line reason>).

Board vs tree: <any task whose status disagrees with the code, and which way — or "board
matches the tree">.

Slices: <the locally provable work> first; <anything needing a push, a publish or an absent
optional dependency> last, and only on your sanction.
Decisions I need before starting: <the ones that change the shape of the work, with a
recommendation each — or "none">.

Per task, "done" means: acceptance criteria verified against the code, one focused conventional
commit, `bun test` and `bun run typecheck` green, task moved in Backlog, and the mirror tables
in `.backlog/docs/` updated in the same commit.
Need from you: confirm the scope (or pick differently) before I start.
```

## 3. Task hand-off (end of a task, before the commit)

```
task_<n> — <title>

Changed: <file> (<one line why>), …
Evidence: <the command that proves it> → <result>. Suite <N> passing (was <N₀>).
Gates: lint ✓ typecheck ✓ test ✓ commitlint ✓
Acceptance criteria: <k>/<total> verified — <the unverified one and what proof it needs, or
"all">.
Left undone: <anything no criterion covers, or "nothing">.
```

## 4. Session report (after the work lands)

```
Session report — <date>

Merged: PR #<N> "<squash title>" → main at <sha>.   [or: Pushed straight to main at <sha>.]
Post-merge runs: ci <verdict, duration>, docs <verdict or "not triggered — no matching paths">.
Landed: <commit subject> (<task id>), …

Tasks: task_<n> done — every criterion verified. task_<m> still in_progress — <criterion> needs
<the proof this machine cannot produce: a published package, an installed optional dependency,
a maintainer's click>.
Suite on merged main: <N> passing (was <N₀>). Gates: lint, typecheck, test green.

Backlog now: <A> done, <B> in_progress, <C> ready. Mirror tables in `.backlog/docs/` updated: <yes/which>.
Open, in the order a session should consider them: <task id — one line on why it is next, or
what blocks it>, …
Environment facts for next time: <what a fresh install will print, what is still not installed,
what the lockfile does>.

Next session should start with:

> <template 1, filled in>
```

A session that produced ideas but no commits still writes a report — "Merged: nothing" is a
result. The ideas go into the Backlog as task descriptions or new tasks, never into the files
they speculate about, and the opening prompt says what the next session should decide first.
