/**
 * The vitest adapter, executed by the real vitest runner (`task_008`).
 *
 * This file is collected by `bun run test:runners` via vitest, not by bun test. It proves
 * injection, teardown, and skipping all work through vitest's own API rather than a
 * double.
 */

import * as vitest from "vitest";
import { describe, expect } from "vitest";
import { vitestSuite } from "@/src/adapters/vitest";

const order: string[] = [];

const suite = vitestSuite(
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
		broken: () => ({
			value: "broken",
			teardown: () => {
				order.push("broken");
				throw new Error("cleanup exploded");
			},
		}),
	},
	vitest as unknown as Parameters<typeof vitestSuite>[1],
);

describe("vitest adapter", () => {
	suite.test("injects a fixture", ({ greeting }) => {
		expect(greeting).toBe("hello");
	});

	suite.test(
		"never constructs a fixture the body does not read",
		({ greeting }) => {
			// `first` and `second` declare teardowns that record into `order`. Reading neither
			// must leave the log untouched, which the next test asserts.
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
		// vitest runs tests in declaration order within a file, so the previous test's
		// teardown has already run by the time this one starts.
		expect(order).toEqual(["body", "second", "first"]);
	});

	suite.skip("registers a skipped test", "no database in this environment");
});
