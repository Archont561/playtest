# Playtest for TypeScript

Polyglot test framework with the batteries included: dependency-injected fixtures, strict
LIFO teardown, property-based testing, and code-first BDD for TypeScript.

## Installation
```bash
bun add -d @archont561/playtest
```

## Quick Start
```typescript
import { bunSuite } from "@archont561/playtest/adapters/bun";

const suite = bunSuite({
	db: () => ({ value: connect() }),
	tmpdir: () => ({ value: mkdtempSync(...) }),
});

suite.test("reads the row", ({ db, tmpdir }) => {
	// `db` and `tmpdir` are injected by name, in the order written.
});
```

Fixtures are injected by **destructuring**, as in Playwright's runner: the pattern names
the fixtures, so the fixture set is visible at the call site. A fixture the body does not
read is never constructed.

## Runner Support

The engine does not import a test runner. A runner is described by its `test` and `skip`
functions, so the same suite runs wherever those exist.

| Runner | Status | Import |
| :--- | :--- | :--- |
| bun test | Verified in this repository's CI | `@archont561/playtest/adapters/bun` |
| vitest | Documented, not yet verified in CI | `@archont561/playtest/adapters/vitest` |
| jest | Documented, not yet verified in CI | `@archont561/playtest/adapters/jest` |

"Documented" means the adapter's shape follows the runner's published API, but this
repository does not install that runner, so nothing here exercises it. The fixture
lifecycle underneath is shared with the bun adapter and is covered by tests.

Another runner works without an adapter file, because a `RunnerHooks` value is just the
functions:

```typescript
import { createSuite } from "@archont561/playtest";
import * as myRunner from "my-test-runner";

const suite = createSuite(fixtures, {
	name: "my-runner",
	test: (title, fn) => myRunner.test(title, fn),
	skip: (title) => myRunner.test.skip(title),
});
```

## Teardown

Fixtures may return a `teardown` callback. Three properties hold, each covered by a test:

- Teardown runs in reverse resolution order, whatever order the body destructured in.
- Teardown runs when the body throws, and when it rejects.
- One failing teardown does not stop the others, and it does not become the headline: if
  the body also failed, the body's error is still the one the runner reports, with the
  cleanup failures attached to it.

Teardown hooks are synchronous. A cleanup that must be awaited cannot be ordered strictly
on a single call, so the engine does not pretend to await it.

## Skipping

```typescript
const suite = bunSuite({});

suite.skip("renders a scene", "requires a GPU");
```

Skipping is **definition-time**, matching `test_skip!` in the Rust macros: call it where
tests are declared. bun test and jest cannot skip a test that has already started, so a
runtime skip is not part of the portable contract. vitest also offers `context.skip()`
inside a body; use it directly in a vitest-only suite.

## Status

This package ships the version contract and the runner-agnostic fixture engine described
above. Code-first BDD and property-based testing land through
[Milestone M5](https://github.com/Archont561/playtest/blob/main/backlog/docs/milestones/M5-v0.5.0-polyglot-conformance.md).
The README tracks the same contract as the other eight targets; it does not promise more
than has shipped.
