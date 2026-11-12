import { expect, test } from "bun:test";
import { spawn, spawnSync } from "node:child_process";
import {
	chmodSync,
	copyFileSync,
	mkdirSync,
	mkdtempSync,
	rmSync,
	writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const THIS_DIR = dirname(fileURLToPath(import.meta.url));
const LOADER_SRC = resolve(THIS_DIR, "../loader/playtest.cjs");
const NODE_BIN = "node";

interface SandboxLayout {
	readonly root: string;
	readonly loaderJs: string;
	readonly platformDir: string;
	readonly binPath: string;
}

function createWrapperSandbox(options?: {
	readonly includePlatformPkg?: boolean;
	readonly executableMode?: number;
	readonly scriptBody?: string;
}): SandboxLayout {
	const includePlatform = options?.includePlatformPkg ?? true;
	const mode = options?.executableMode ?? 0o755;
	const root = mkdtempSync(join(tmpdir(), "playtest-loader-test-"));
	const wrapperDir = join(root, "node_modules", "@archont561", "playtest");
	const wrapperBinDir = join(wrapperDir, "bin");
	mkdirSync(wrapperBinDir, { recursive: true });

	// Wrapper explicitly declares `"type": "module"` (Spec 0012 §4.3 rule 5).
	writeFileSync(
		join(wrapperDir, "package.json"),
		JSON.stringify(
			{
				name: "@archont561/playtest",
				version: "0.1.0",
				type: "module",
				bin: { playtest: "bin/playtest.js" },
			},
			null,
			2,
		),
	);
	writeFileSync(
		join(wrapperBinDir, "package.json"),
		`${JSON.stringify({ type: "commonjs" })}\n`,
	);
	const loaderJs = join(wrapperBinDir, "playtest.js");
	copyFileSync(LOADER_SRC, loaderJs);
	chmodSync(loaderJs, 0o755);

	const platformKey = `${process.platform}-${process.arch}`;
	const platformDir = join(
		root,
		"node_modules",
		"@archont561",
		`playtest-${platformKey}`,
	);
	const binPath = join(
		platformDir,
		process.platform === "win32" ? "playtest.exe" : "playtest",
	);

	if (includePlatform) {
		mkdirSync(platformDir, { recursive: true });
		writeFileSync(
			join(platformDir, "package.json"),
			JSON.stringify(
				{
					name: `@archont561/playtest-${platformKey}`,
					version: "0.1.0",
					os: [process.platform],
					cpu: [process.arch],
				},
				null,
				2,
			),
		);
		const body =
			options?.scriptBody ??
			'#!/usr/bin/env sh\necho "playtest-fake args:$*"\nif [ -n "$FAKE_EXIT" ]; then exit "$FAKE_EXIT"; fi\nexit 0\n';
		writeFileSync(binPath, body);
		chmodSync(binPath, mode);
	}

	return { root, loaderJs, platformDir, binPath };
}

test("missing platform package exits 3 with package name, fix, and no stack trace", () => {
	const sb = createWrapperSandbox({ includePlatformPkg: false });
	try {
		const res = spawnSync(NODE_BIN, [sb.loaderJs, "version"], {
			encoding: "utf8",
		});
		expect(res.status).toBe(3);
		expect(res.stderr).toContain(
			`@archont561/playtest-${process.platform}-${process.arch}`,
		);
		expect(res.stderr).toContain("Fix:");
		expect(res.stderr).not.toContain("    at ");
	} finally {
		rmSync(sb.root, { recursive: true, force: true });
	}
});

test("unsupported platform exits 3 with fix and no stack trace", () => {
	const sb = createWrapperSandbox({ includePlatformPkg: false });
	try {
		const res = spawnSync(NODE_BIN, [sb.loaderJs, "version"], {
			encoding: "utf8",
			env: {
				...process.env,
				PLAYTEST_TEST_PLATFORM: "sunos",
				PLAYTEST_TEST_ARCH: "sparc64",
			},
		});
		expect(res.status).toBe(3);
		expect(res.stderr).toContain('unsupported platform "sunos-sparc64"');
		expect(res.stderr).toContain("cargo install playtest-cli --locked");
		expect(res.stderr).not.toContain("    at ");
	} finally {
		rmSync(sb.root, { recursive: true, force: true });
	}
});

test("non-executable binary exits 3 with chmod fix and no stack trace", () => {
	if (process.platform === "win32") {
		return;
	}
	const sb = createWrapperSandbox({ executableMode: 0o644 });
	try {
		const res = spawnSync(NODE_BIN, [sb.loaderJs, "version"], {
			encoding: "utf8",
		});
		expect(res.status).toBe(3);
		expect(res.stderr).toContain("not executable");
		expect(res.stderr).toContain("chmod +x");
		expect(res.stderr).not.toContain("    at ");
	} finally {
		rmSync(sb.root, { recursive: true, force: true });
	}
});

test("wrapper with type module executes loader and passes arguments and exit codes through", () => {
	if (process.platform === "win32") {
		return;
	}
	const sb = createWrapperSandbox();
	try {
		for (const disableExecve of ["0", "1"]) {
			for (const code of [0, 1, 2, 3, 42]) {
				const res = spawnSync(NODE_BIN, [sb.loaderJs, "sync", "--check"], {
					encoding: "utf8",
					env: {
						...process.env,
						PLAYTEST_DISABLE_EXECVE: disableExecve,
						FAKE_EXIT: String(code),
					},
				});
				expect(res.status).toBe(code);
				expect(res.stdout).toContain("playtest-fake args:sync --check");
			}
		}
	} finally {
		rmSync(sb.root, { recursive: true, force: true });
	}
});

test("signal termination forwards SIGTERM to binary and terminates loader with SIGTERM", async () => {
	if (process.platform === "win32") {
		return;
	}
	const sb = createWrapperSandbox({
		scriptBody:
			'#!/usr/bin/env node\nprocess.stdout.write("READY\\n");\nsetInterval(() => {}, 1000);\n',
	});
	try {
		const result = await new Promise<{
			code: number | null;
			signal: NodeJS.Signals | null;
		}>((resolvePromise, rejectPromise) => {
			const child = spawn(NODE_BIN, [sb.loaderJs], {
				env: { ...process.env, PLAYTEST_DISABLE_EXECVE: "1" },
				stdio: ["ignore", "pipe", "pipe"],
			});
			let killed = false;
			child.stdout.on("data", (chunk: Buffer) => {
				if (!killed && chunk.toString("utf8").includes("READY")) {
					killed = true;
					child.kill("SIGTERM");
				}
			});
			child.on("error", rejectPromise);
			child.on("exit", (code, signal) => {
				resolvePromise({ code, signal });
			});
		});
		expect(result.signal).toBe("SIGTERM");
		expect(result.code).toBeNull();
	} finally {
		rmSync(sb.root, { recursive: true, force: true });
	}
});
