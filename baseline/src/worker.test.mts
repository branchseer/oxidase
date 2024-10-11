import * as assert from "node:assert/strict";
import { describe, it } from "node:test";
import { checkJs, processTs } from "./worker.mjs";

describe("checkJs", async () => {
	it("detects script", () => {
		assert.equal(checkJs("let a = 1", null), "script");
	});
	it("detects module", () => {
		assert.equal(checkJs("export const a = 1", null), "module");
	});
	it("checks invalid", () => {
		assert.equal(checkJs("a +", null), null);
	});
	it("check invalid 2", () => {
		assert.equal(
			checkJs(
				`class C {
    async static {
        // something
    }

    public static {
        // something
    }
			}`,
				null,
			),
			null,
		);
	});
	it.only("rejects jsx", () => {
		assert.equal(checkJs("let a = <div />", null), null);
	});
	it("enforces module", () => {
		assert.equal(checkJs("let a = 1", "module"), "module");
	});
	it("enforces script", () => {
		assert.equal(checkJs("export let a = 1", "script"), "script");
	});
});

describe("processTs", () => {
	it("transpiles and formats", () => {
		assert.deepEqual(processTs("let a: string = 1;;;", null), {
			ts: "let a: string = 1;\n;\n;\n",
			js: "let a = 1;\n",
			kind: "script",
		});
	});

	it("rejects jsx", () => {
		assert.equal(processTs("let a = <div>", null), null);
	});
	it("detects module", () => {
		assert.deepEqual(processTs("export let a: string = 1", null), {
			ts: "export let a: string = 1;\n",
			js: "export let a = 1;\n",
			kind: "module",
		});
	});
	it("enforces module", () => {
		assert.deepEqual(processTs("let a: string = 1", "module"), {
			ts: "let a: string = 1;\n",
			js: "let a = 1;\n",
			kind: "module",
		});
	});
	it("strips enum/namespace", () => {
		assert.deepEqual(
			processTs(
				`
export enum A {}
export namespace B {}
enum C {}
namespace D {}
declare enum E {}
declare namespace F {}
function Foo() {
	enum A {}
}
`,
				"module",
			),
			{
				ts: "declare enum E {\n}\ndeclare namespace F { }\nfunction Foo() {\n}\n",
				js: "function Foo() {\n}\n",
				kind: "module",
			},
		);
	});
	it("reports syntax error", () => {
		assert.equal(processTs("a +", null), null);
	});
});
