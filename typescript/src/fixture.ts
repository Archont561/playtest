/**
 * The runner-agnostic fixture core (`Spec 0005 §1.1`).
 *
 * Nothing in this module imports a test runner. The same three guarantees the Rust engine
 * and the Python bridge provide are enforced here, and a runner adapter (see
 * `playtest/adapters`) supplies the few things only a runner can do: how to skip, and how
 * to attach a cleanup to the runner's own lifecycle.
 *
 * - **Explicit resolution:** a fixture is constructed when a test asks for it, in the
 *   order the test asks, and never merely because it was registered.
 * - **Strict LIFO teardown:** cleanup runs in reverse resolution order.
 * - **Failure safety:** one failing cleanup does not stop the remaining cleanups, and it
 *   never replaces the test's own failure.
 *
 * Teardown is modelled on synchronous hooks. A framework whose cleanup must be awaited
 * cannot guarantee strict reverse order in a single call, so it accepts ordered
 * `Promise`-returning hooks here instead of deferring the ordering to the runtime.
 */

/** What a cleanup does after the fixture value exists. */
export type TeardownHook = () => void;

/** What happened while releasing one test's fixtures. */
export interface TeardownReport {
	/** How many hooks ran to completion. */
	readonly ran: number;
	/** One entry per hook that threw, in the order the hooks ran. */
	readonly failures: readonly string[];
	/** Whether every hook completed without throwing. */
	readonly isClean: boolean;
}

/** A fixture whose cleanup failed, collected so every other cleanup still runs. */
export class TeardownError extends Error {
	readonly report: TeardownReport;

	constructor(report: TeardownReport) {
		const detail = report.failures
			.map((failure) => `  - ${failure}`)
			.join("\n");
		super(`${report.failures.length} fixture teardown(s) failed:\n${detail}`);
		this.name = "TeardownError";
		this.report = report;
	}
}

/** Best-effort text for whatever a cleanup threw. */
function describeThrown(thrown: unknown): string {
	if (thrown instanceof Error) {
		return thrown.message;
	}
	return String(thrown);
}

/**
 * Teardown hooks for one test, released in reverse creation order.
 *
 * A stack holds callables rather than values, so a fixture's cleanup can be registered
 * the moment its value exists. Releasing is idempotent: a caller that tears down
 * explicitly still leaves the stack empty for the runner's own finalizer.
 */
export class FixtureStack {
	readonly #hooks: TeardownHook[] = [];

	/** How many fixtures are still awaiting teardown. */
	get size(): number {
		return this.#hooks.length;
	}

	/** Whether every fixture has been released. */
	get isEmpty(): boolean {
		return this.#hooks.length === 0;
	}

	/** Register a teardown hook. Hooks run in reverse registration order. */
	push(hook: TeardownHook): void {
		this.#hooks.push(hook);
	}

	/**
	 * Resolve a fixture and register its cleanup.
	 *
	 * `teardown` is optional because most fixtures are pure reads with nothing to release.
	 */
	setup<T>(construct: () => T, teardown?: TeardownHook): T {
		const value = construct();
		if (teardown !== undefined) {
			this.push(teardown);
		}
		return value;
	}

	/**
	 * Run every hook in reverse registration order.
	 *
	 * A hook that throws is recorded and the loop continues, matching the Rust engine:
	 * one broken cleanup must not strand the fixtures created before it.
	 */
	teardownAll(): TeardownReport {
		const failures: string[] = [];
		let ran = 0;
		while (this.#hooks.length > 0) {
			const hook = this.#hooks.pop();
			if (hook === undefined) {
				break;
			}
			try {
				hook();
				ran += 1;
			} catch (thrown) {
				failures.push(describeThrown(thrown));
			}
		}
		return { ran, failures, isClean: failures.length === 0 };
	}
}
