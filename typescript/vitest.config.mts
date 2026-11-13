import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

/**
 * Runs only `tests/runners/vitest.ts` under vitest, so the vitest adapter is exercised by
 * the real runner rather than a double. bun's lane keeps `tests/`, so the two suites never
 * import each other's runner.
 */
export default defineConfig({
	resolve: {
		alias: {
			"@": fileURLToPath(new URL(".", import.meta.url)),
		},
	},
	test: {
		include: ["tests/runners/vitest.ts"],
		environment: "node",
	},
});
