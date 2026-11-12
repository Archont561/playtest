# Playtest positioning: the contract across nine ecosystems

Status: applied. This document records the positioning Playtest settles on, the claims it
refuses to make, and the checks that keep the one-line description from drifting again.

---

## 0. What the repository now says

Every ownable metadata surface opens with the canonical line, enforced by
`_check_canonical_description` in `automation/repo_check.py`:

> Polyglot test framework with the batteries included: dependency-injected fixtures,
> strict LIFO teardown, property-based testing, and code-first BDD for <target>.

The GitHub About description and topics are maintained outside the repository, and the
values to set are in §3 and §4. Nothing else in this document is pending work.

---

## 1. First, the hard part: three of your four claims need rework

Your framing was: **next-gen polyglot test framework, batteries included, all kinds of
tests + coverage + CI + AI-native**. Three of those four survive. One of them, and one
phrase, do not.

### "Next-gen" — drop it

It is unfalsifiable, every competitor uses it, and it dates the moment you ship `v1.0.0`.
It also invites the question "newer than what?" which you answer better with a mechanism
than an adjective.

### "All kinds of tests against code possible" — this contradicts your own caveats

`backlog/docs/caveats.md` §5 explicitly places these out of scope: load and stress
testing, security scanning (SAST/DAST), chaos engineering, A/B testing, canary
deployments, coverage-guided fuzzing engines, race and interleaving model checking,
manual test management.

If the README promises "all kinds of tests" and a visitor finds `caveats.md` two clicks
later, the README loses. The credible move is the opposite: **state the boundary
proudly.** A framework that names its non-goals reads as engineered. One that claims
everything reads as vapour.

What you can honestly say instead, and it is arguably a stronger claim, is a *typed*
inventory of what is in scope:

| In core (deterministic, pre-deployment, assertion-shaped) | Delegated to native tools, orchestrated by the CLI | Out of scope, stated in `caveats.md` |
| :--- | :--- | :--- |
| Fixture/dependency injection, code-first BDD, property-based testing with shrinking, parameterized cases, snapshot/golden tests, record/replay HTTP, hermetic isolation, deterministic clock/seed, cross-language conformance | Coverage (LCOV aggregation, Spec 0008 §4) and mutation testing (Spec 0011) | Load, security scanning, chaos, A/B, canary, fuzzing engines, model checking |

### "Coverage reports" and "CI integration" — true, but weaker and stronger than you think

**Coverage is currently a repo capability, not a user capability.** `automation/merge_lcov.py`
merges nine target tracefiles into one `coverage/lcov.info`, and CI dogfoods it into a
single Codecov report. But user-facing unified coverage is Spec 0008 §4 at milestone M5
(`v0.5.0`) and is not shipped. Say "the same pipeline that produces this repository's
single coverage report becomes `playtest test --coverage` in M5", not "coverage reports"
as a present-tense feature.

**CI is genuinely undersold in your current copy.** Spec 0008 §7 and ADR 0012 give you
something most test tools fumble: four *distinct* exit codes — `0` pass, `1` test failure,
`2` usage error, `3` environment failure. Plus versioned JSON with `schema_version`,
stdout carrying data only, no prompts and no ANSI in machine modes, a JUnit rendering of
the same document, and failing property tests shipping their replay seed. A pipeline can
tell "your code is broken" from "your toolchain is unqualified" from "you called it wrong".
That is the single most quotable thing in the whole repo and it appears nowhere in the
current README.

### "AI-native" — the biggest risk, and you already have the honest version

As stated, "AI-native" reads as an LLM feature and invites "where is the model?". You
have no MCP server, no LLM assistance, no prompt surface — and adding those words would
commit you to a spec that does not exist yet.

But ADR 0012 already names **AI agents as a first-class consumer class**, and specifies
exactly what such a consumer needs: stable documented exit codes, versioned JSON schemas,
a non-interactive stream discipline, no colour and no prompts in machine modes, reproduce
commands in failures, and guidance reachable from `playtest version` rather than only
from prose docs.

That property has a better name than "AI-native": **agent-legible** (or
*agent-operable*). It is true today, it is specified, and it does not require you to own a
model. You can then say: "Playtest is built for three consumers: humans, CI, and agents."

Two paths, pick one:

- **Path A (recommended): claim agent-legibility now.** Honest, specified, defensible.
  If you later ship an MCP server it becomes a natural extension, not a correction.
- **Path B: commit to a literal AI feature.** Then it needs its own spec plus milestone
  (an MCP surface over `list`/`doctor`/`test`) before the word "AI-native" appears in the
  description. Until that lands, the word is a liability.

---

## 2. The refined concept

### Positioning statement

> **Playtest is one contract across nine ecosystems.** A single pinned CLI is the version
> authority; each language gets an idiomatic SDK generated from the same behavioral
> contract for fixtures, BDD, property testing, and deterministic isolation. One command,
> one coverage report, one machine contract for CI and agents.

### The four proof pillars (all traceable to a spec or to code)

| Pillar | Claim | Evidence |
| :--- | :--- | :--- |
| **One version authority** | CLI and all nine SDKs share one version; mixing versions is an error, not a supported matrix | ADR 0015, ADR 0018, Spec 0013 §6 |
| **Batteries, literally** | The `playtest` binary carries all nine SDK sources (solid-brotli); `init` and `sync` write them into your repo as reviewable, committed mirrors | `crates/playtest-assets`, Spec 0013 |
| **One behavioral contract** | The same fixture, LIFO teardown, BDD, property-testing and isolation semantics in every target | Specs 0001–0005 |
| **Machine contract first** | Versioned JSON, JUnit, four distinct exit codes, no prompts in machine modes, replay seeds on failure | Spec 0008 §7, Spec 0014, ADR 0012 |

### What "batteries included" should mean for you

Do not use it the Python way (standard library completeness). Use it in the one sense that
is *literally true and unusual*: **the binary carries the SDKs**. No other polyglot test
tool ships nine language SDKs inside one executable and materialises them into the
consumer's repo. That is a battery. Lead with that.

### The one-line ladder

- **One line:** Nine languages, one test contract, one CLI.
- **One paragraph:** the positioning statement above.
- **Three bullets:** one version authority; one behavioral contract; one machine contract.

### Maturity line (non-negotiable, put it above the fold)

Today on `main`: `playtest version`, `init`, `sync`; committed SDK mirrors for nine
languages; multi-channel binary packaging; the full two-phase release pipeline. Not yet:
the fixture engine (M2), BDD and property testing (M3), isolation helpers (M4), and
`test`/`coverage`/`list`/`doctor`/`mutation` orchestration (M5).

Your current README already hedges this well in the Language Targets section. Keep that
discipline and move a one-line version of it to the top, because a visitor's first action
is `playtest test` and that command returns a usage error until M5.

---

## 3. Repository description (GitHub About)

Current (156 chars):

> CLI-first polyglot testing framework with dependency-injected fixtures, strict LIFO teardown, code-first BDD, and property-based testing across 9 languages.

Problems: leads with an implementation detail ("CLI-first"), buries the differentiator
(nine ecosystems, one contract), and says nothing about coverage, CI, or agents.

**Recommended (163 chars):**

> Polyglot test framework, batteries included: fixtures, BDD, property tests, mutation and coverage across 9 languages, plus a machine contract for CI and AI agents.

**Tighter variant (140 chars)** if you want it to fit a narrow About box:

> Polyglot test framework: fixtures, BDD, property and mutation tests, merged coverage for 9 languages, and a CI/agent-first machine contract.

**Value-first variant (120 chars)**, best if you prefer benefit over inventory:

> Nine languages, one test contract: fixtures, BDD, property tests, unified coverage, and agent-legible CLI output for CI.

Pick one. Do not keep the current text alongside a new one anywhere — a drifted
description is worse than a vague one.

The About box is repository settings, not a tracked file, so no gate here can hold it in
place. A maintainer with admin rights sets it once:

```bash
gh repo edit Archont561/playtest \
  --description "Polyglot test framework, batteries included: fixtures, BDD, property tests, mutation and coverage across 9 languages, plus a machine contract for CI and AI agents."
```

---

## 4. Topics

You have 19 of GitHub's 20 topics. Two months from now the missing one matters more than
`vcr` does.

| Action | Topic | Why |
| :--- | :--- | :--- |
| **Add** | `test-automation` | 10,508 repos; the highest-traffic term that describes you and is absent |
| **Add** | `code-coverage` | 675 repos; directly matches the coverage pillar |
| **Drop** | `vcr` | 129 repos; describes one fixture, not the product |
| **Keep** | `testing`, `test-framework`, `polyglot`, `fixtures`, `bdd`, `property-based-testing`, `snapshot-testing`, `cli`, `developer-tools`, `rust`, `python`, `typescript`, `c`, `cpp`, `csharp`, `java`, `golang`, `r` | All accurate |
| **Skip** | `ai`, `ai-native`, `llm`, `mcp`, `agent-native` | `ai` alone carries 193,794 repos and signals a claim you cannot yet prove. Revisit only after Path B ships. |
| **Skip** | `continuous-integration` / `ci` | 11,404 and 4,848 repos; topical for many repos, rarely searched by people choosing a test framework |

`csharp` (115,507 repos) over `c-sharp` (12,050) — keep what you have.

Final set: 18 kept + `test-automation` + `code-coverage` = **20**. One command, also a
maintainer action:

```bash
gh repo edit Archont561/playtest \
  --add-topic test-automation \
  --add-topic code-coverage \
  --remove-topic vcr
```

Verify the result reads back as 20 topics before calling the positioning work closed.

---

## 5. README

### Mechanics you must respect first

- `README.md` is **generated** from `automation/readme/README.ejs`; do not hand-edit it.
  Edit the EJS, then `pixi run doit readme`. CI fails on drift via `doit readme_check`.
- Vale strict runs over `README.md`. Banned by the vendored styles: weasel words
  (`very`, `clearly`, `simply`, `various`, `usually`, `mostly`, `significantly`, …),
  `http://` links, double spaces after sentence punctuation. Canonical casing is enforced
  for `GitHub Actions`, `Semgrep`, `Vale`, `Alint`, `Pixi`, `Bun`, `pytest`, `ast-grep`.
- The `<pre>` block holds `<!-- playtest-banner -->`; the FIGlet wordmark is injected by
  the generator. Leave the marker alone.

### What to change, in priority order

**1. Replace the blockquote with a claim + a proof table.** The current text ("Universal
testing framework across nine language ecosystems… one shared behavioral contract for
dependency-injected fixtures, failure-safe LIFO teardown…") is accurate but reads as a
feature list with no spine. Suggested:

```markdown
> Nine languages. One test contract. One CLI that pins them to a single version.
>
> Playtest is a polyglot test framework with the batteries included: the `playtest`
> binary carries the SDK sources for all nine targets and writes them into your
> repository as committed, reviewable mirrors. Fixtures, code-first BDD, property-based
> testing, and deterministic isolation share one behavioral contract across Rust,
> Python, TypeScript, C, C++, C#, Go, Java, and R.
```

**2. Add a status line directly under it.** Two lines, no hedging adjectives:

```markdown
**Status:** `version`, `init`, and `sync` ship today, with committed SDK mirrors and
multi-channel binary packaging. The fixture engine, BDD, property testing, and
`playtest test` / `coverage` / `doctor` / `mutation` orchestration are scheduled through
[Milestone M5](backlog/docs/milestones/M5-v0.5.0-polyglot-conformance.md).
```

**3. Add a "Built for three consumers" section.** This is where CI and agent-legibility
pay off, and it is the section no competitor can copy without your exit-code design:

```markdown
## Built for three consumers

| Consumer | What Playtest gives it |
| :--- | :--- |
| **Humans** | One command across nine ecosystems, with `pretty` output that renders the same data the machine formats carry |
| **CI** | JUnit XML, versioned JSON (`schema_version` on every document), and four distinct exit codes: `0` pass, `1` test failure, `2` usage error, `3` environment failure |
| **Agents** | No prompts and no ANSI in machine modes, stdout carrying data only, failing property tests shipping their replay seed, and guidance reachable from `playtest version` instead of only from prose |
```

Note the exit-code row: it is the strongest sentence in the README and it is currently
absent.

**4. Replace "Language Targets" with a shipped/planned split.** Keep the table, add one
column, and keep the honest note:

| Language | Ecosystem | Directory | Package / Artifact | Today |
| :--- | :--- | :--- | :--- | :--- |
| Rust | Cargo / crates.io | `crates/` | five crates including `playtest-cli` | package scaffold |
| … | | | | |

**5. Add a short "Scope" section that links to `caveats.md`.** Counter-intuitive, and it
will do more for credibility than any superlative:

```markdown
## Scope

Playtest covers deterministic, pre-deployment, assertion-shaped testing. Load testing,
security scanning, chaos engineering, A/B testing, and fuzzing engines are deliberate
non-goals, and coverage and mutation results are delegated to native tools and
normalized, never reimplemented. The full boundary is in
[`backlog/docs/caveats.md`](backlog/docs/caveats.md).
```

**6. Fix the `Usage` section.** It currently shows `cargo run -p playtest-cli --locked`,
which is a maintainer command, not a user one. Show the three shipped commands and label
the rest as upcoming:

```sh
playtest version            # print the pinned CLI + SDK version
playtest init --lang rust   # write playtest.toml and the SDK mirrors
playtest sync --check       # fail when mirrors drift from the pinned version
```

**7. Delete the `🏠 Homepage` heading.** It points at `#readme`, which is the page the
reader is already on.

---

## 6. The one-liner had 13 copies, and they had drifted

Positioning text is duplicated across every package manifest. Before this change the
copies used three vocabularies — "universal", "Playwright-style", "Playwright-inspired" —
and none matched the GitHub About line.

All 13 ownable surfaces now open with one canonical clause and append their target:
`Cargo.toml`, `crates/playtest/Cargo.toml`, `package.json`, `typescript/package.json`,
`python/pyproject.toml`, `c/conanfile.py`, `cpp/conanfile.py`, `java/pom.xml`,
`csharp/src/Playtest/Playtest.csproj`, and the C, C# and C#-packaged READMEs.

`automation/repo_check.py` enforces the clause through `_check_canonical_description`, so
the next drift fails `doit fast` with a message naming the file and the expected lead. The
guard has unit coverage in `automation/tests/test_governance.py`.

The "Playwright-style" anchor still does real work — instant comprehension for one
feature — so it stays in the specs and ADRs, where it can be qualified, and leaves the
package descriptions, where it undersold nine ecosystems as "one library, but Rust".

## 6a. A command with no specification

Setting the About box also means promising `code-coverage` as a topic, and that prompted an
audit of every `playtest <command>` mention across the design documents. It found that the
README, this record, and the milestone summary all named a `coverage` orchestration command
arriving in M5, while no specification, task, or milestone deliverable defined one.
[Spec 0008 §4](../../backlog/docs/specs/0008-cross-language-cli-and-toolchain-detection.md)
described only an LCOV merge performed by a "shared CLI/automation layer", and the only
implementation of that merge is `automation/merge_lcov.py`, a build task.

A command promised in a README that no contract defines is the same class of defect as a
description that drifts between files: the copy outran the design. Two repairs were
possible — withdraw the claim, or make the design true. The second was chosen, because the
claim is defensible and the merge already exists in a form worth standardizing:

- [Spec 0015](../../backlog/docs/specs/0015-coverage-collection-and-reporting.md) defines
  `playtest coverage`: native instrumentation per target, LCOV normalization and merge,
  `--min-lines` / `--min-branches` thresholds, and its relationship to `playtest test`,
  which already merges LCOV in its report phase.
- [Spec 0008 §3](../../backlog/docs/specs/0008-cross-language-cli-and-toolchain-detection.md)
  lists the command in the grammar and points to Spec 0015; §4 names both surfaces that
  perform the merge.
- `task_068` tracks the implementation in M5, with the acceptance criteria the command
  needs, and the M5 milestone carries the matching deliverable.

The lesson is narrower than "write more specs". The README was written from the milestone
prose, and the milestone prose was the thing that never named `coverage`. A claim entering
user-facing copy should be traceable to a specification line, and this document is where
that traceability was missing.

## 7. Summary of applied changes

| Item | Change |
| :--- | :--- |
| **Concept** | Coherence play, not breadth play: one contract, nine ecosystems, one version authority |
| **Drop** | "Next-gen"; "all kinds of tests"; "AI-native" unless you ship an AI feature |
| **Keep** | "Batteries included", redefined as "the binary carries the nine SDKs" |
| **Add** | Agent-legibility: exit codes, versioned JSON, machine-mode discipline |
| **Description** | Applied to all 13 package manifests; GitHub About value in §3 |
| **Topics** | +`test-automation` +`code-coverage`, −`vcr` → 20 of 20 |
| **README** | Applied: new blockquote, status line, "Why one contract" table, "three consumers" table, scope section, shipped/planned column, real usage commands |
| **Beyond the README** | Applied: 13 package descriptions unified on the canonical lead, guarded by `repo_check._check_canonical_description` |
| **Coverage claim** | Repaired: the `coverage` command the README promised is now defined by Spec 0015 and tracked by `task_068` in M5, instead of being an untraceable claim (§6a) |

---
