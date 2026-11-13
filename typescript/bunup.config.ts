import { defineConfig } from "bunup";

export default defineConfig({
	entry: [
		"src/index.ts",
		"src/adapters/bun.ts",
		"src/adapters/vitest.ts",
		"src/adapters/jest.ts",
	],
	outDir: "dist",
	format: ["esm"],
	target: "node",
	dts: true,
	splitting: false,
	sourcemap: "none",
	clean: true,
	preferredTsconfig: "./tsconfig.json",
});
