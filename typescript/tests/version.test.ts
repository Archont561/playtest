import { expect, test } from "bun:test";
import { VERSION } from "../src/index.js";

test("version is exposed and matches 0.1.0", () => {
	expect(VERSION).toBe("0.1.0");
});
