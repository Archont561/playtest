/**
 * vitest adapter (`task_008`).
 *
 * Status: **documented, not verified in this repository's CI.** The `ts` environment does
 * not install vitest, so the shape below follows vitest's documented API rather than a
 * test. The fixture lifecycle it relies on is shared with the bun adapter and is covered
 * by the core tests.
 *
 * The runner functions are passed in by the caller, so importing this module never pulls
 * vitest into a bun suite. That is what keeps the core runner-agnostic.
 *
 * vitest is the one targeted runner that can also skip *dynamically*, by calling
 * `context.skip()` inside a body. This adapter uses the definition-time form anyway, so a
 * suite behaves the same under bun test and jest. Reach for `context.skip()` directly when
 * a vitest-only suite needs a runtime decision.
 */

import { createSuite, type FixtureMap, type Suite } from "@/src/facade";
import type { RunnerHooks } from "@/src/runner";

/** The vitest surface this adapter needs, structurally typed so vitest stays optional. */
export interface VitestLike {
	readonly test: (name: string, fn: () => void | Promise<void>) => unknown;
	/** `test.skip`, which vitest exposes as the `.skip` property on `test`. */
	readonly skip: (name: string) => unknown;
}

/** Describe a vitest module as a runner. */
export function vitestRunner(vitest: VitestLike): RunnerHooks {
	return {
		name: "vitest",
		test: (title, fn) => vitest.test(title, fn),
		skip: (name) => vitest.skip(name),
	};
}

/** Build a Playtest suite that runs on vitest. */
export function vitestSuite<M extends FixtureMap>(
	fixtures: M,
	vitest: VitestLike,
): Suite<M> {
	return createSuite(fixtures, vitestRunner(vitest));
}
