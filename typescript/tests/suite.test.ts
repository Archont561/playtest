/**
 * Suite behaviour under a runner (`task_008`).
 *
 * A small runner double captures what `createSuite` registers and then runs the captured
 * body, which makes ordering deterministic. A real bun run cannot prove LIFO order across
 * sibling tests, because the runner decides when each test starts.
 *
 * The bun adapter itself is exercised against the real runner in `bun-adapter.test.ts`.
 */

import { beforeEach, describe, expect, test } from "bun:test";
import { createSuite } from "@/src/facade";
import type { RunnerHooks } from "@/src/runner";

interface Registered {
	readonly title: string;
	readonly body?: () => void | Promise<void>;
}

/** A runner double that records registrations instead of running them. */
function fakeRunner(name = "fake"): {
	readonly hooks: RunnerHooks;
	readonly registered: Registered[];
	/** Run the body registered for `title`, as the real runner would. */
	readonly run: (title: string) => Promise<void>;
} {
	const registered: Registered[] = [];
	const hooks: RunnerHooks = {
		name,
		test: (title, body) => {
			registered.push({ title, body });
		},
		skip: (title) => {
			registered.push({ title });
		},
	};
	return {
		hooks,
		registered,
		run: async (title) => {
			const entry = registered.find((item) => item.title === title);
			if (entry?.body === undefined) {
				throw new Error(`no runnable test registered as "${title}"`);
			}
			await entry.body();
		},
	};
}

describe("createSuite", () => {
	let order: string[];

	beforeEach(() => {
		order = [];
	});

	test("injects fixtures in the order the body destructures them", async () => {
		const runner = fakeRunner();
		const suite = createSuite(
			{
				first: () => ({ value: "a" }),
				second: () => ({ value: "b" }),
			},
			runner.hooks,
		);

		suite.test("pair", ({ first, second }) => {
			expect([first, second]).toEqual(["a", "b"]);
			order.push("body");
		});
		await runner.run("pair");

		expect(order).toEqual(["body"]);
	});

	test("never constructs a fixture the body does not read", async () => {
		const runner = fakeRunner();
		const suite = createSuite(
			{
				requested: () => ({ value: "used" }),
				unrequested: () => {
					order.push("constructed-unrequested");
					return { value: "never" };
				},
			},
			runner.hooks,
		);

		suite.test("explicit", ({ requested }) => {
			expect(requested).toBe("used");
		});
		await runner.run("explicit");

		expect(order).toEqual([]);
	});

	test("tears down in strict LIFO order", async () => {
		const runner = fakeRunner();
		const suite = createSuite(
			{
				alpha: () => ({ value: "a", teardown: () => order.push("alpha") }),
				beta: () => ({ value: "b", teardown: () => order.push("beta") }),
			},
			runner.hooks,
		);

		suite.test("lifo", ({ alpha, beta }) => {
			expect([alpha, beta]).toEqual(["a", "b"]);
			order.push("body");
		});
		await runner.run("lifo");

		expect(order).toEqual(["body", "beta", "alpha"]);
	});

	test("tears down even when the body throws", async () => {
		const runner = fakeRunner();
		const suite = createSuite(
			{ res: () => ({ value: "v", teardown: () => order.push("res") }) },
			runner.hooks,
		);

		suite.test("fails", ({ res }) => {
			expect(res).toBe("v");
			throw new Error("body blew up");
		});
		await expect(runner.run("fails")).rejects.toThrow("body blew up");

		expect(order).toEqual(["res"]);
	});

	test("tears down even when an async body rejects", async () => {
		const runner = fakeRunner();
		const suite = createSuite(
			{ res: () => ({ value: "v", teardown: () => order.push("res") }) },
			runner.hooks,
		);

		suite.test("rejects", async ({ res }) => {
			expect(res).toBe("v");
			await Promise.reject(new Error("async blew up"));
		});
		await expect(runner.run("rejects")).rejects.toThrow("async blew up");

		expect(order).toEqual(["res"]);
	});

	test("one broken cleanup does not stop the others", async () => {
		const runner = fakeRunner();
		const suite = createSuite(
			{
				alpha: () => ({ value: "a", teardown: () => order.push("alpha") }),
				broken: () => ({
					value: "b",
					teardown: () => {
						order.push("broken");
						throw new Error("cleanup exploded");
					},
				}),
				gamma: () => ({ value: "c", teardown: () => order.push("gamma") }),
			},
			runner.hooks,
		);

		suite.test("body", ({ alpha, broken, gamma }) => {
			expect([alpha, broken, gamma]).toEqual(["a", "b", "c"]);
		});
		await expect(runner.run("body")).rejects.toThrow("cleanup exploded");

		expect(order).toEqual(["gamma", "broken", "alpha"]);
	});

	test("a cleanup failure does not hide the body failure", async () => {
		const runner = fakeRunner();
		const suite = createSuite(
			{
				broken: () => ({
					value: "b",
					teardown: () => {
						throw new Error("cleanup exploded");
					},
				}),
			},
			runner.hooks,
		);

		suite.test("body", () => {
			throw new Error("body blew up");
		});

		// The body's own failure is what the runner sees, which is the guarantee: a
		// cleanup problem must never replace the reason the test failed.
		await expect(runner.run("body")).rejects.toThrow("body blew up");
	});

	test("a fixture read twice is constructed once", async () => {
		let constructions = 0;
		const runner = fakeRunner();
		const suite = createSuite(
			{
				once: () => {
					constructions += 1;
					return { value: `built-${constructions}` };
				},
			},
			runner.hooks,
		);

		suite.test("twice", ({ once }) => {
			expect(once).toBe("built-1");
		});
		await runner.run("twice");

		expect(constructions).toBe(1);
	});

	test("reading an undeclared fixture fails with its name", async () => {
		const runner = fakeRunner();
		const suite = createSuite({ known: () => ({ value: 1 }) }, runner.hooks);

		suite.test("unknown", (resolved) => {
			expect(
				(resolved as unknown as Record<string, unknown>).missing,
			).toBeUndefined();
		});

		await expect(runner.run("unknown")).rejects.toThrow(
			'no fixture named "missing"',
		);
	});

	test("a second read of the same fixture returns the cached value", async () => {
		let constructions = 0;
		const runner = fakeRunner();
		const suite = createSuite(
			{
				cached: () => {
					constructions += 1;
					return { value: constructions };
				},
			},
			runner.hooks,
		);

		suite.test("reads twice", (resolved) => {
			const first = resolved.cached;
			const second = resolved.cached;
			expect(second).toBe(first);
			expect(constructions).toBe(1);
		});

		await runner.run("reads twice");
	});

	test("answers a membership check without constructing anything", async () => {
		let constructed = false;
		const runner = fakeRunner();
		const suite = createSuite(
			{
				declared: () => {
					constructed = true;
					return { value: "v" };
				},
			},
			runner.hooks,
		);

		suite.test("probes", (resolved) => {
			const probe = resolved as unknown as Record<string, unknown>;
			expect("declared" in probe).toBe(true);
			expect("absent" in probe).toBe(false);
			expect(constructed).toBe(false);
		});

		await runner.run("probes");
	});

	test("ignores symbol access, which no fixture is keyed by", async () => {
		const runner = fakeRunner();
		const suite = createSuite({ real: () => ({ value: 1 }) }, runner.hooks);

		suite.test("symbols", (resolved) => {
			const probe = resolved as unknown as Record<symbol, unknown>;
			expect(probe[Symbol.iterator]).toBeUndefined();
		});

		await runner.run("symbols");
	});

	test("attaches cleanup failures to the body failure without replacing it", async () => {
		const runner = fakeRunner();
		const suite = createSuite(
			{
				broken: () => ({
					value: "b",
					teardown: () => {
						throw new Error("cleanup exploded");
					},
				}),
			},
			runner.hooks,
		);

		const bodyError = new Error("body blew up");
		suite.test("both fail", ({ broken }) => {
			expect(broken).toBe("b");
			throw bodyError;
		});

		await expect(runner.run("both fail")).rejects.toThrow("body blew up");
		// The headline stays the body's failure, and the cleanup detail rides alongside.
		expect(
			(bodyError as Error & { teardownFailures?: string[] }).teardownFailures,
		).toEqual(["cleanup exploded"]);
	});

	test("logs cleanup failures when the body threw a non-object", async () => {
		const runner = fakeRunner();
		const suite = createSuite(
			{
				broken: () => ({
					value: "b",
					teardown: () => {
						throw new Error("cleanup exploded");
					},
				}),
			},
			runner.hooks,
		);

		// A thrown string cannot carry a field, so the failures must not vanish silently.
		suite.test("throws a string", ({ broken }) => {
			expect(broken).toBe("b");
			throw "plain string failure";
		});

		const original = console.error;
		const logged: string[] = [];
		console.error = (message: string) => {
			logged.push(message);
		};
		try {
			await expect(runner.run("throws a string")).rejects.toBe(
				"plain string failure",
			);
		} finally {
			console.error = original;
		}

		expect(logged.join("\n")).toContain("cleanup exploded");
	});

	test("skips at definition time, through the runner", () => {
		const runner = fakeRunner();
		const suite = createSuite({}, runner.hooks);

		suite.skip("not now", "needs a database");

		expect(runner.registered).toEqual([{ title: "not now" }]);
	});

	test("the core never imports a runner", async () => {
		// The claim this package makes to be runner-agnostic is only true if the core
		// stays free of runner imports. Enforce it, so a convenient import cannot quietly
		// re-couple the engine to bun.
		const { readFileSync } = await import("node:fs");
		const { dirname, join } = await import("node:path");
		const { fileURLToPath } = await import("node:url");
		const srcDir = join(
			dirname(dirname(fileURLToPath(import.meta.url))),
			"src",
		);
		const core = ["fixture.ts", "runner.ts", "facade.ts", "index.ts"];

		for (const file of core) {
			const source = readFileSync(join(srcDir, file), "utf8");
			for (const runner of [
				'from "bun:test"',
				'from "vitest"',
				'from "@jest/globals"',
			]) {
				expect(source).not.toContain(runner);
			}
		}
	});

	test("a runner without skip refuses to report a skip as a pass", () => {
		const runner = fakeRunner("bare-runner");
		const suite = createSuite(
			{},
			{ name: runner.hooks.name, test: runner.hooks.test },
		);

		expect(() => suite.skip("cannot", "unsupported")).toThrow(/bare-runner/);
		expect(runner.registered).toEqual([]);
	});
});
