/**
 * vitest adapter (`task_008`).
 *
 * Verified against vitest 5 in this repository's CI (`sdk-test` runs `tests/runners/vitest.ts`).
 *
 * vitest has no top-level `skip` export; the skip is `test.skip`. Unlike jest it accepts
 * the name alone, and it also accepts a callback, so the same call shape works on both
 * runners and the adapter stays uniform.
 *
 * vitest is the one targeted runner that can also skip *dynamically*, by calling
 * `context.skip()` inside a body. This adapter uses the definition-time form anyway, so a
 * suite behaves the same under bun test and jest. Reach for `context.skip()` directly when
 * a vitest-only suite needs a runtime decision.
 */

import { createSuite, type FixtureMap, type Suite } from "@/src/facade";
import type { RunnerHooks } from "@/src/runner";

/** A vitest test function, including the `.skip` property vitest hangs off it. */
export interface VitestTestFn {
	(name: string, fn: () => void | Promise<void>): unknown;
	/** vitest accepts the name alone, unlike jest. */
	readonly skip: (name: string) => unknown;
}

/** The vitest surface this adapter needs, structurally typed so vitest stays optional. */
export interface VitestLike {
	readonly test: VitestTestFn;
}

/** Describe a vitest module as a runner. */
export function vitestRunner(vitest: VitestLike): RunnerHooks {
	return {
		name: "vitest",
		test: (title, fn) => vitest.test(title, fn),
		skip: (name, reason) =>
			vitest.test.skip(reason === undefined ? name : `${name}: ${reason}`),
	};
}

/** Build a Playtest suite that runs on vitest. */
export function vitestSuite<M extends FixtureMap>(
	fixtures: M,
	vitest: VitestLike,
): Suite<M> {
	return createSuite(fixtures, vitestRunner(vitest));
}
