/**
 * The bun adapter against the real bun test runner (`task_008`).
 *
 * Everything else in the suite uses a runner double. This file is the one place that
 * proves the adapter wires into an actual runner: it writes a small project to a temp
 * directory, runs `bun test` on it, and reads the reported outcome.
 *
 * A subprocess is necessary because the interesting assertions are about what the *runner*
 * reports — a skip must appear as a skip, and a failing body must appear as a failure.
 */

import { describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const SDK_ROOT = dirname(dirname(fileURLToPath(import.meta.url)));

interface RunResult {
	readonly status: number | null;
	readonly output: string;
}

/** Run a Playtest suite file through the real bun test runner. */
function runUnderBun(source: string): RunResult {
	const dir = mkdtempSync(join(tmpdir(), "playtest-bun-"));
	try {
		writeFileSync(join(dir, "suite.test.ts"), source, "utf8");
		const result = spawnSync("bun", ["test", join(dir, "suite.test.ts")], {
			encoding: "utf8",
			env: { ...process.env, NODE_PATH: join(SDK_ROOT, "node_modules") },
		});
		return {
			status: result.status,
			output: `${result.stdout ?? ""}${result.stderr ?? ""}`,
		};
	} finally {
		rmSync(dir, { recursive: true, force: true });
	}
}

/** The import preamble every generated suite needs, pointing back at this package. */
const preamble = `import { bunSuite } from "${join(SDK_ROOT, "src", "adapters", "bun.ts")}";\n`;

describe("bun adapter", () => {
	test("injects a fixture and reports a pass", () => {
		const result = runUnderBun(`${preamble}
const suite = bunSuite({
	db: () => ({ value: "connected" }),
});

suite.test("uses the fixture", ({ db }) => {
	if (db !== "connected") throw new Error(\`unexpected: \${db}\`);
});
`);

		expect(result.output).toContain("(pass) uses the fixture");
		expect(result.status).toBe(0);
	});

	test("registers a skip the runner reports as skipped", () => {
		const result = runUnderBun(`${preamble}
const suite = bunSuite({});

suite.skip("needs a database", "no database in this environment");
`);

		// bun reports the title and counts it as a skip, never as a pass.
		expect(result.output).toContain("(skip) needs a database");
		expect(result.output).toContain("1 skip");
		expect(result.output).toContain("0 fail");
	});

	test("reports a failing body as a failure", () => {
		const result = runUnderBun(`${preamble}
const suite = bunSuite({
	res: () => ({ value: "v" }),
});

suite.test("fails loudly", ({ res }) => {
	if (res === "v") throw new Error("body blew up");
});
`);

		expect(result.output).toContain("(fail) fails loudly");
		expect(result.output).toContain("body blew up");
		expect(result.status).not.toBe(0);
	});

	test("runs teardown and still fails on a broken cleanup", () => {
		const result = runUnderBun(`${preamble}
const suite = bunSuite({
	broken: () => ({
		value: "b",
		teardown: () => {
			throw new Error("cleanup exploded");
		},
	}),
});

suite.test("body passes", ({ broken }) => {
	if (broken !== "b") throw new Error("bad injection");
});
`);

		// The body passed, but the cleanup failure is reported rather than swallowed.
		expect(result.output).toContain("cleanup exploded");
		expect(result.status).not.toBe(0);
	});

	test("an unrequested fixture is never constructed", () => {
		const result = runUnderBun(`${preamble}
let constructed = false;
const suite = bunSuite({
	unused: () => {
		constructed = true;
		return { value: "never" };
	},
	used: () => ({ value: "used" }),
});

suite.test("reads one", ({ used }) => {
	if (used !== "used") throw new Error("bad injection");
});

suite.test("asserts the other never ran", () => {
	if (constructed) throw new Error("unused fixture was constructed");
});
`);

		expect(result.output).toContain("(pass) asserts the other never ran");
		expect(result.status).toBe(0);
	});
});
