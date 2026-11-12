import { readFileSync } from "node:fs";

export interface PackageManifest {
	readonly version: string;
}

export function discoverVersion(): string {
	const raw = readFileSync(new URL("../package.json", import.meta.url), "utf8");
	const manifest = JSON.parse(raw) as PackageManifest;
	return manifest.version;
}
