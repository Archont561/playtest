/**
 * @archont561/playtest
 * Playwright-style testing framework for TypeScript.
 */

import { discoverVersion } from "@/src/version";

export type { PackageManifest } from "@/src/version";

export const VERSION: string = discoverVersion();
export const defaultExport: { readonly VERSION: string } = { VERSION };
export default defaultExport;
