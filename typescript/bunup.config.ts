import { defineConfig } from "bunup";

export default defineConfig({
	entry: ["src/index.ts"],
	outDir: "dist",
	format: ["esm"],
	target: "node",
	dts: true,
	splitting: false,
	sourcemap: "none",
	clean: true,
	preferredTsconfig: "./tsconfig.json",
});
