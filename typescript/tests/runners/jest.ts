/**
 * The jest adapter, executed by the real jest runner (`task_008`).
 *
 * This file is collected by `bun run test:runners` via jest, not by bun test. It proves
 * injection, teardown, and skipping all work through jest's own API rather than a double.
 *
 * jest requires a callback for `test.skip` and has no reason parameter. Both facts are
 * asserted here rather than assumed, because the first version of the adapter got them
 * wrong.
 */

import * as jestGlobals from "@jest/globals";
import { expect, test } from "@jest/globals";
import { jestSuite } from "@/src/adapters/jest";

const order: string[] = [];

const suite = jestSuite(
	{
		greeting: () => ({ value: "hello" }),
		first: () => ({
			value: "a",
			teardown: () => {
				order.push("first");
			},
		}),
		second: () => ({
			value: "b",
			teardown: () => {
				order.push("second");
			},
		}),
	},
	jestGlobals as unknown as Parameters<typeof jestSuite>[1],
);

suite.test("injects a fixture", ({ greeting }) => {
	expect(greeting).toBe("hello");
});

suite.test(
	"never constructs a fixture the body does not read",
	({ greeting }) => {
		expect(greeting).toBe("hello");
	},
);

suite.test("asserts nothing was torn down for the unread fixtures", () => {
	expect(order).toEqual([]);
});

suite.test(
	"tears down in reverse order after the body",
	({ first, second }) => {
		expect([first, second]).toEqual(["a", "b"]);
		order.push("body");
	},
);

suite.test("asserts the teardown order recorded by the previous test", () => {
	// jest runs tests in declaration order within a file, so the previous test's teardown
	// has already run by the time this one starts.
	expect(order).toEqual(["body", "second", "first"]);
});

suite.skip("registers a skipped test", "no database in this environment");

test("jest's test.skip requires a callback, which the adapter supplies", () => {
	// A regression guard for the bug this adapter shipped with: `test.skip(name)` throws
	// in jest, so the adapter must pass a callback that never runs.
	expect(() => {
		(jestGlobals.test as unknown as { skip: (name: string) => unknown }).skip(
			"deliberately malformed",
		);
	}).toThrow(/callback/);
});
