/**
 * jest adapter (`task_008`).
 *
 * Status: **documented, not verified in this repository's CI.** Jest is not installed in
 * the `ts` environment, so the shape below follows jest's documented API rather than a
 * test. The fixture lifecycle it relies on is shared with the bun adapter and is covered
 * by the core tests.
 *
 * jest's `test.skip` is definition-time, like bun's and vitest's, so the portable skip
 * contract holds unchanged. This is the reason the Playtest skip is declaration-level
 * rather than something a running test can do.
 */

import { createSuite, type FixtureMap, type Suite } from "@/src/facade";
import type { RunnerHooks } from "@/src/runner";

/** The jest surface this adapter needs, structurally typed so jest stays optional. */
export interface JestLike {
	readonly test: (name: string, fn: () => void | Promise<void>) => unknown;
	/** `test.skip`, which jest exposes as the `.skip` property on `test`. */
	readonly skip: (name: string) => unknown;
}

/** Describe a jest module as a runner. */
export function jestRunner(jest: JestLike): RunnerHooks {
	return {
		name: "jest",
		test: (title, fn) => jest.test(title, fn),
		skip: (name) => jest.skip(name),
	};
}

/** Build a Playtest suite that runs on jest. */
export function jestSuite<M extends FixtureMap>(
	fixtures: M,
	jest: JestLike,
): Suite<M> {
	return createSuite(fixtures, jestRunner(jest));
}
