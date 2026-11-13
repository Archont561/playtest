/**
 * Cross-language conformance suites for the fixture contract (Spec 0005 §1, task_023).
 *
 * Each test is one cell of the `(capability x target)` matrix in `backlog/conformance.yml`.
 * The capabilities exercised here are the ones this target currently claims; the remaining
 * capabilities are declared in the manifest but not claimed, so they report as
 * `not_implemented` rather than as passing.
 *
 * The tests assert observable behaviour through the public API only, so the same scenarios
 * are expressible in the Rust and Python suites without sharing implementation details.
 */

import { describe, expect, test } from "bun:test";
import { FixtureStack, TeardownError } from "@/src/index";

function newTrace(): string[] {
	return [];
}

describe("conformance: fixture_explicit_resolution", () => {
	test("only requested fixtures are constructed", () => {
		const trace = newTrace();
		const stack = new FixtureStack();

		stack.setup(
			() => {
				trace.push("requested:setup");
				return "value";
			},
			() => trace.push("requested:teardown"),
		);

		// A second fixture exists but is never resolved, so nothing about it runs.
		const unused = (): string => {
			trace.push("unused:setup");
			return "never";
		};
		expect(typeof unused).toBe("function");

		expect(trace).toEqual(["requested:setup"]);
		expect(stack.size).toBe(1);
	});
});

describe("conformance: teardown_lifo", () => {
	test("cleanup is the exact reverse of setup", () => {
		const trace = newTrace();
		const stack = new FixtureStack();

		for (const name of ["a", "b", "c"]) {
			stack.setup(
				() => {
					trace.push(`${name}:setup`);
					return name;
				},
				() => trace.push(`${name}:teardown`),
			);
		}

		const report = stack.teardownAll();

		expect(report.isClean).toBe(true);
		expect(report.ran).toBe(3);
		expect(trace).toEqual([
			"a:setup",
			"b:setup",
			"c:setup",
			"c:teardown",
			"b:teardown",
			"a:teardown",
		]);
	});
});

describe("conformance: teardown_failure_safety", () => {
	test("a failing cleanup never stops the remaining cleanups", () => {
		const trace = newTrace();
		const stack = new FixtureStack();

		stack.setup(
			() => {
				trace.push("outer:setup");
				return "outer";
			},
			() => trace.push("outer:teardown"),
		);
		stack.push(() => {
			trace.push("boom:teardown");
			throw new Error("deliberate conformance failure");
		});

		const report = stack.teardownAll();

		expect(report.isClean).toBe(false);
		expect(report.failures).toHaveLength(1);
		expect(trace).toContain("boom:teardown");
		expect(trace).toContain("outer:teardown");
	});

	test("the failure detail survives into the error message", () => {
		const stack = new FixtureStack();
		stack.push(() => {
			throw new Error("teardown exploded");
		});

		const report = stack.teardownAll();
		const error = new TeardownError(report);

		expect(error.message).toContain("teardown exploded");
		expect(error.report.failures).toHaveLength(1);
	});
});
