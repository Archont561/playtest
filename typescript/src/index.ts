/**
 * @archont561/playtest
 * Playwright-style testing framework for TypeScript.
 */

import { readFileSync } from "node:fs";

interface PackageManifest {
	readonly version: string;
}

function discoverVersion(): string {
	const raw = readFileSync(new URL("../package.json", import.meta.url), "utf8");
	const manifest = JSON.parse(raw) as PackageManifest;
	return manifest.version;
}

export const VERSION: string = discoverVersion();
export default { VERSION };
