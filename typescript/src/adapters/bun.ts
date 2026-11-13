/**
 * bun test adapter (`task_008`).
 *
 * bun test is the runner this repository verifies against, so it is the adapter with an
 * end-to-end test. The adapter is thin by design: the fixture lifecycle lives in the
 * runner-agnostic core, and this file supplies only bun's runner-specific facts.
 *
 * Playwright needs no adapter file: its `test` and `test.skip` already satisfy
 * `RunnerHooks`, so a Playwright suite passes them straight to `createSuite`.
 */

import { test } from "bun:test";
import { createSuite, type FixtureMap, type Suite } from "@/src/facade";
import type { RunnerHooks } from "@/src/runner";

/**
 * bun test as a runner description.
 *
 * bun's skip is `test.skip(label)`, a definition-time registration with no reason
 * parameter, so the reason is not carried into bun's own output. The Playtest `skip` call
 * still requires it, and the runner-agnostic core keeps it for reporters that ask.
 */
export const bunTest: RunnerHooks = {
	name: "bun test",
	test: (title, fn) => test(title, fn),
	skip: (name) => test.skip(name),
};

/** Build a Playtest suite that runs on bun test. */
export function bunSuite<M extends FixtureMap>(fixtures: M): Suite<M> {
	return createSuite(fixtures, bunTest);
}
