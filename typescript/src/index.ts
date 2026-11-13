/**
 * @archont561/playtest
 * Polyglot test framework with the batteries included: dependency-injected fixtures,
 * strict LIFO teardown, property-based testing, and code-first BDD for TypeScript.
 */

import { discoverVersion } from "@/src/version";

export type { PackageManifest } from "@/src/version";

export const VERSION: string = discoverVersion();
export const defaultExport: { readonly VERSION: string } = { VERSION };
export default defaultExport;
