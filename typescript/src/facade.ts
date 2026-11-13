/**
 * The entry point a Playtest test uses inside a runner (`task_008`).
 *
 * `createSuite` keeps the runner's own `test` call: it wraps a normal test body with
 * fixture resolution and teardown, so the runner still discovers, runs, and reports the
 * test. That is what lets one suite run under bun test, vitest, or jest — the fixtures are
 * the Playtest part and the runner stays in charge of everything else.
 *
 * Fixtures are resolved through a lazy accessor. A destructuring pattern such as
 * `({ db, tmp }) => ...` reads `db` then `tmp`, which is exactly the request order; a
 * fixture the body never reads is never constructed, so explicit resolution holds by
 * construction rather than by convention.
 */

import { FixtureStack, TeardownError } from "@/src/fixture";
import { MissingSkipError, type RunnerHooks } from "@/src/runner";

/** What a fixture builder returns: the value, and optionally how to release it. */
export interface FixtureProvider<T> {
	readonly value: T;
	/** Omit for fixtures that need no cleanup. */
	readonly teardown?: () => void;
}

/** A fixture defined once and resolved per test. */
export type FixtureDefinition<T> = () => FixtureProvider<T>;

/** The fixtures a suite declares, keyed by the name a test destructures. */
export type FixtureMap = Record<string, FixtureDefinition<unknown>>;

/** The fixture values a test body receives. */
export type Resolved<M extends FixtureMap> = {
	readonly [K in keyof M]: ReturnType<M[K]>["value"];
};

/** The test body shape a suite accepts. */
export type TestBody<M extends FixtureMap> = (
	resolved: Resolved<M>,
) => void | Promise<void>;

/**
 * Resolve fixtures on first access and remember them for teardown.
 *
 * The accessor is what makes the ordering guarantees observable: the first read of a name
 * constructs it and pushes its cleanup, so the stack order is the request order. A second
 * read returns the same value rather than constructing the fixture twice.
 */
function lazyFixtures<M extends FixtureMap>(
	fixtures: M,
	stack: FixtureStack,
): Resolved<M> {
	const resolved = new Map<string, unknown>();

	return new Proxy({} as Resolved<M>, {
		get(_target, property: string | symbol): unknown {
			if (typeof property === "symbol") {
				return undefined;
			}
			if (resolved.has(property)) {
				return resolved.get(property);
			}
			const definition = fixtures[property];
			if (definition === undefined) {
				throw new Error(
					`Playtest: no fixture named "${property}" is defined for this suite`,
				);
			}
			const provider = definition();
			if (provider.teardown !== undefined) {
				stack.push(provider.teardown);
			}
			resolved.set(property, provider.value);
			return provider.value;
		},
		has(_target, property: string | symbol): boolean {
			return typeof property === "string" && property in fixtures;
		},
	});
}

/**
 * Record cleanup failures on the error that already explains why the test failed.
 *
 * The body's failure stays the headline; the cleanup detail is attached beside it. When
 * the thrown value is not an object it cannot carry a field, so the failures are written
 * to stderr rather than silently lost.
 */
function attachTeardownFailures(
	thrown: unknown,
	failures: readonly string[],
): void {
	if (typeof thrown === "object" && thrown !== null) {
		Object.assign(thrown, { teardownFailures: failures });
		return;
	}
	console.error(
		`Playtest: fixture teardown also failed:\n${failures.join("\n")}`,
	);
}

/** A suite bound to one set of fixtures and one runner. */
export interface Suite<M extends FixtureMap> {
	/** Register a test that receives the fixtures it destructures. */
	readonly test: (title: string, body: TestBody<M>) => void;
	/**
	 * Register a skipped test.
	 *
	 * Definition-time, not runtime: call it where tests are declared, as the Rust
	 * `test_skip!` macro is used. bun test and jest cannot skip a running test, and an API
	 * that only worked on vitest would split a suite the moment it moved runner.
	 */
	readonly skip: (title: string, reason: string) => void;
}

/**
 * Bind a fixture map and a runner into the `test`/`skip` pair a suite uses.
 *
 * Teardown runs whether the body returns, throws, or rejects, and a failing cleanup is
 * reported without replacing the body's own failure.
 */
export function createSuite<M extends FixtureMap>(
	fixtures: M,
	runner: RunnerHooks,
): Suite<M> {
	return {
		test(title, body) {
			runner.test(title, async () => {
				const stack = new FixtureStack();
				let bodyFailure: { thrown: unknown } | undefined;
				try {
					await body(lazyFixtures(fixtures, stack));
				} catch (thrown) {
					// Recorded rather than rethrown here, so the cleanup still runs and the
					// body's failure keeps its place as the reason the test failed.
					bodyFailure = { thrown };
				}
				const report = stack.teardownAll();
				if (bodyFailure !== undefined) {
					// Cleanup failures are attached to the body failure instead of replacing
					// it: a cleanup problem must never become the headline. A thrown
					// non-object cannot carry the field, so the failures are logged instead
					// of being dropped silently.
					if (!report.isClean) {
						attachTeardownFailures(bodyFailure.thrown, report.failures);
					}
					throw bodyFailure.thrown;
				}
				if (!report.isClean) {
					throw new TeardownError(report);
				}
			});
		},
		skip(title, reason) {
			if (runner.skip === undefined) {
				throw new MissingSkipError(runner.name, { name: title, reason });
			}
			runner.skip(title, reason);
		},
	};
}
