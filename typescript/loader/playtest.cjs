#!/usr/bin/env node
"use strict";

const { spawn } = require("node:child_process");
const fs = require("node:fs");
const { createRequire } = require("node:module");
const path = require("node:path");

const EXIT_ENVIRONMENT_FAILURE = 3;

const PLATFORM_PACKAGES = Object.freeze({
	"linux-x64": "@archont561/playtest-linux-x64",
	"linux-arm64": "@archont561/playtest-linux-arm64",
	"darwin-x64": "@archont561/playtest-darwin-x64",
	"darwin-arm64": "@archont561/playtest-darwin-arm64",
	"win32-x64": "@archont561/playtest-win32-x64",
});

function failEnvironment(message) {
	process.stderr.write(`playtest: ${message}\n`);
	process.exit(EXIT_ENVIRONMENT_FAILURE);
}

function resolvePlatformBinary() {
	const hostPlatform = process.env.PLAYTEST_TEST_PLATFORM || process.platform;
	const hostArch = process.env.PLAYTEST_TEST_ARCH || process.arch;
	const key = `${hostPlatform}-${hostArch}`;
	const pkgName = PLATFORM_PACKAGES[key];
	if (!pkgName) {
		const supported = Object.keys(PLATFORM_PACKAGES).join(", ");
		failEnvironment(
			`unsupported platform "${key}" (supported: ${supported}). ` +
				"Fix: build from source with `cargo install playtest-cli --locked`.",
		);
	}

	const resolverBase = process.env.PLAYTEST_LOADER_BASE || __filename;
	const req = createRequire(resolverBase);
	let pkgJsonPath;
	try {
		pkgJsonPath = req.resolve(`${pkgName}/package.json`);
	} catch {
		failEnvironment(
			`platform binary package "${pkgName}" is not installed. ` +
				'Fix: reinstall "@archont561/playtest" without "--omit=optional" from a clean tree.',
		);
	}

	const binFile = hostPlatform === "win32" ? "playtest.exe" : "playtest";
	const binPath = path.join(path.dirname(pkgJsonPath), binFile);

	if (!fs.existsSync(binPath)) {
		failEnvironment(
			`binary "${binPath}" is missing from installed package "${pkgName}". ` +
				`Fix: reinstall "${pkgName}" or "@archont561/playtest".`,
		);
	}

	if (hostPlatform !== "win32") {
		try {
			fs.accessSync(binPath, fs.constants.X_OK);
		} catch {
			failEnvironment(
				`binary "${binPath}" from package "${pkgName}" is not executable. ` +
					`Fix: run \`chmod +x "${binPath}"\` or reinstall "${pkgName}".`,
			);
		}
	}

	return { binPath, hostPlatform, pkgName };
}

function runBinary() {
	const { binPath, hostPlatform, pkgName } = resolvePlatformBinary();
	const args = process.argv.slice(2);

	if (
		hostPlatform !== "win32" &&
		typeof process.execve === "function" &&
		process.env.PLAYTEST_DISABLE_EXECVE !== "1"
	) {
		try {
			process.execve(binPath, [binPath, ...args], process.env);
		} catch (err) {
			const detail = err?.code ?? "execve failed";
			failEnvironment(
				`failed to execute "${binPath}" from "${pkgName}" (${detail}). ` +
					`Fix: reinstall "${pkgName}" or build with \`cargo install playtest-cli --locked\`.`,
			);
		}
	}

	const child = spawn(binPath, args, { stdio: "inherit" });
	const signals =
		hostPlatform === "win32"
			? ["SIGINT", "SIGTERM"]
			: ["SIGINT", "SIGTERM", "SIGHUP", "SIGQUIT"];
	const listeners = new Map();

	for (const sig of signals) {
		const handler = () => {
			if (!child.killed) {
				try {
					child.kill(sig);
				} catch {
					// Child already exited.
				}
			}
		};
		listeners.set(sig, handler);
		process.on(sig, handler);
	}

	child.on("error", (err) => {
		const detail = err?.code ?? "spawn failed";
		failEnvironment(
			`failed to start "${binPath}" from "${pkgName}" (${detail}). ` +
				`Fix: reinstall "${pkgName}".`,
		);
	});

	child.on("exit", (code, signal) => {
		for (const [sig, handler] of listeners) {
			process.removeListener(sig, handler);
		}
		if (signal) {
			process.kill(process.pid, signal);
			return;
		}
		process.exit(code === null ? 0 : code);
	});
}

runBinary();
