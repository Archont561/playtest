/**
 * @archont561/playtest
 * Polyglot test framework with the batteries included: dependency-injected fixtures,
 * strict LIFO teardown, property-based testing, and code-first BDD for TypeScript.
 *
 * The fixture engine is runner-agnostic: import the core from here, and a runner
 * description from `@archont561/playtest/adapters/<runner>`.
 */

import { discoverVersion } from "@/src/version";

export type {
	FixtureDefinition,
	FixtureMap,
	FixtureProvider,
	Resolved,
	Suite,
	TestBody,
} from "@/src/facade";
export { createSuite } from "@/src/facade";
export type { TeardownHook, TeardownReport } from "@/src/fixture";
export { FixtureStack, TeardownError } from "@/src/fixture";
export type {
	RunnerHooks,
	RunnerSkipFn,
	RunnerTestFn,
	SkipRequest,
} from "@/src/runner";
export { MissingSkipError } from "@/src/runner";

export const VERSION: string = discoverVersion();
export const defaultExport: { readonly VERSION: string } = { VERSION };
export default defaultExport;
