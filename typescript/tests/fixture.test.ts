/**
 * Fixture engine unit tests (`task_008`, `Spec 0005 §1.1`).
 *
 * These cover the runner-agnostic core directly: ordering, failure isolation, and the
 * report shape. End-to-end behaviour under a real runner lives in `suite.test.ts`.
 */

import { describe, expect, test } from "bun:test";
import { FixtureStack, TeardownError } from "@/src/fixture";

describe("FixtureStack", () => {
	test("releases in reverse creation order", () => {
		const order: string[] = [];
		const stack = new FixtureStack();

		stack.push(() => order.push("a"));
		stack.push(() => order.push("b"));
		stack.push(() => order.push("c"));

		const report = stack.teardownAll();

		expect(order).toEqual(["c", "b", "a"]);
		expect(report.ran).toBe(3);
		expect(report.isClean).toBe(true);
	});

	test("a throwing cleanup does not stop the others", () => {
		const order: string[] = [];
		const stack = new FixtureStack();

		stack.push(() => order.push("first"));
		stack.push(() => {
			order.push("broken");
			throw new Error("cleanup exploded");
		});
		stack.push(() => order.push("last"));

		const report = stack.teardownAll();

		expect(order).toEqual(["last", "broken", "first"]);
		expect(report.isClean).toBe(false);
		expect(report.failures).toEqual(["cleanup exploded"]);
		// The count is of hooks that completed, so the broken one is not included.
		expect(report.ran).toBe(2);
	});

	test("collects every failure rather than only the first", () => {
		const stack = new FixtureStack();

		stack.push(() => {
			throw new Error("one");
		});
		stack.push(() => {
			throw new Error("two");
		});

		const report = stack.teardownAll();

		expect(report.failures).toEqual(["two", "one"]);
	});

	test("teardown is idempotent", () => {
		const order: string[] = [];
		const stack = new FixtureStack();
		stack.push(() => order.push("once"));

		stack.teardownAll();
		const second = stack.teardownAll();

		expect(order).toEqual(["once"]);
		expect(second.ran).toBe(0);
		expect(second.isClean).toBe(true);
	});

	test("reports its size and emptiness", () => {
		const stack = new FixtureStack();
		expect(stack.size).toBe(0);
		expect(stack.isEmpty).toBe(true);

		stack.push(() => undefined);
		expect(stack.size).toBe(1);
		expect(stack.isEmpty).toBe(false);

		stack.teardownAll();
		expect(stack.isEmpty).toBe(true);
	});

	test("describes a non-Error throw", () => {
		const stack = new FixtureStack();
		stack.push(() => {
			// A cleanup may throw anything, not only an Error.
			throw "plain string";
		});

		expect(stack.teardownAll().failures).toEqual(["plain string"]);
	});

	test("setup resolves a value and registers its cleanup", () => {
		const order: string[] = [];
		const stack = new FixtureStack();

		const value = stack.setup(
			() => "resolved",
			() => order.push("torn down"),
		);

		expect(value).toBe("resolved");
		expect(stack.size).toBe(1);
		stack.teardownAll();
		expect(order).toEqual(["torn down"]);
	});

	test("setup without a teardown registers nothing", () => {
		const stack = new FixtureStack();

		expect(stack.setup(() => 42)).toBe(42);
		expect(stack.isEmpty).toBe(true);
	});
});

describe("TeardownError", () => {
	test("lists every failure in its message", () => {
		const error = new TeardownError({
			ran: 0,
			failures: ["first", "second"],
			isClean: false,
		});

		expect(error.message).toContain("2 fixture teardown(s) failed");
		expect(error.message).toContain("first");
		expect(error.message).toContain("second");
		expect(error.report.failures).toEqual(["first", "second"]);
		expect(error.name).toBe("TeardownError");
	});
});
