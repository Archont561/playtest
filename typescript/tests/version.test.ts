import { expect, test } from "bun:test";
import pkg from "@/package.json";
import { VERSION } from "@/src/index";

test("version is valid semver and matches package.json", () => {
	expect(VERSION).toMatch(/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/);
	expect(VERSION).toBe(pkg.version);
});
