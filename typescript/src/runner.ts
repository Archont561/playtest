/**
 * The seam between the fixture core and a test runner (`Spec 0005 §2`).
 *
 * The core never imports a runner. Instead a runner is described by the few functions
 * only it can provide, which is what lets bun test, vitest, and jest share one engine
 * and lets an unsupported runner work by passing its own functions.
 *
 * Skipping is deliberately **definition-time**, matching `test_skip!` and `skip_if!` in
 * the Rust macros (`Spec 0002 §2`). That is the only form every targeted runner supports:
 * bun test and jest cannot skip a test that has already started, so a runtime skip cannot
 * be part of a portable contract. vitest is the exception — it also offers
 * `context.skip()` from inside a body — but an API that only works on one runner would
 * split a suite the moment it moved. The facade therefore refuses to fake a runtime skip
 * rather than report a skipped test as passed.
 */

/** Information needed to register a skipped test. */
export interface SkipRequest {
	/** The Playtest display name of the skipped test. */
	readonly name: string;
	/** Why it is skipped. Always required: a skip without a reason is not actionable. */
	readonly reason: string;
}

/** The parts of a runner's `test` function the facade needs. */
export type RunnerTestFn = (
	name: string,
	fn: () => void | Promise<void>,
) => unknown;

/** The parts of a runner's skip function the facade needs. */
export type RunnerSkipFn = (name: string, reason?: string) => unknown;

/** A runner described by the functions it exposes. */
export interface RunnerHooks {
	/** A short identifier, used in error messages so a reader knows which runner failed. */
	readonly name: string;
	readonly test: RunnerTestFn;
	/**
	 * Register a skipped test at definition time.
	 *
	 * Omit only for a runner that cannot express a skip at all; asking to skip on such a
	 * runner raises {@link MissingSkipError} instead of silently passing.
	 */
	readonly skip?: RunnerSkipFn;
}

/**
 * Thrown when a skip is requested on a runner that cannot express one.
 *
 * Reporting a skip as a pass would be a false green, the worst outcome for a test
 * framework, so this fails loudly and names the runner that could not comply.
 */
export class MissingSkipError extends Error {
	constructor(runner: string, request: SkipRequest) {
		super(
			`Playtest: the "${runner}" runner cannot register a skipped test.\n` +
				`Skipped: ${request.name}\n` +
				`Reason: ${request.reason}\n` +
				"Give the adapter a `skip` function (most runners expose `test.skip`), or " +
				"remove the test from the suite.",
		);
		this.name = "MissingSkipError";
	}
}
