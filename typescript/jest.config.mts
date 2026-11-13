import type { Config } from "jest";

/**
 * Runs only `tests/runners/jest.ts` under jest, so the jest adapter is exercised by the
 * real runner rather than a double. The `@/` alias is mapped here because jest does not
 * read tsconfig paths, and the repository forbids relative imports in the SDK.
 */
const config: Config = {
	testEnvironment: "node",
	rootDir: ".",
	testMatch: ["<rootDir>/tests/runners/jest.ts"],
	transform: { "^.+\\.tsx?$": ["@swc/jest", {}] },
	moduleNameMapper: { "^@/(.*)$": "<rootDir>/$1" },
};

export default config;
