/**
 * jest adapter (`task_008`).
 *
 * Verified against jest 30 in this repository's CI (`sdk-test` runs `tests/runners/jest.ts`).
 *
 * Two jest facts shape this adapter, both established by running jest rather than reading
 * its docs:
 *
 * - There is no top-level `skip`. The skip is `test.skip`, and jest requires a callback
 *   for it: `test.skip(name)` throws "Missing second argument. It must be a callback
 *   function." The adapter supplies one that never runs.
 * - jest has no parameter for a skip reason. Since a reason is required by the Playtest
 *   API and dropping it would make the skip unexplained, it is appended to the reported
 *   name so it stays visible in jest's output.
 */

import { createSuite, type FixtureMap, type Suite } from "@/src/facade";
import type { RunnerHooks } from "@/src/runner";

/** A jest test function, including the `.skip` property jest hangs off it. */
export interface JestTestFn {
	(name: string, fn: () => void | Promise<void>): unknown;
	/** jest requires a callback here: `test.skip(name)` throws. */
	readonly skip: (name: string, fn: () => void) => unknown;
}

/** The jest surface this adapter needs, structurally typed so jest stays optional. */
export interface JestLike {
	readonly test: JestTestFn;
}

/**
 * The callback jest demands for a skipped test.
 *
 * jest requires a callback even for `test.skip` and will not report a skip without one, so
 * this is the smallest function that satisfies the signature. jest never invokes it.
 */
function notRun(): void {}

/** Describe a jest module as a runner. */
export function jestRunner(jest: JestLike): RunnerHooks {
	return {
		name: "jest",
		test: (title, fn) => jest.test(title, fn),
		skip: (name, reason) =>
			jest.test.skip(
				reason === undefined ? name : `${name}: ${reason}`,
				notRun,
			),
	};
}

/** Build a Playtest suite that runs on jest. */
export function jestSuite<M extends FixtureMap>(
	fixtures: M,
	jest: JestLike,
): Suite<M> {
	return createSuite(fixtures, jestRunner(jest));
}
