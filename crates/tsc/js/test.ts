import { describe, it } from 'node:test'
import assert from 'node:assert/strict'
import { processTs } from "./index.ts";

describe("processTs", () => {
    it('should format and transpile ts', () => {
        const result = processTs("class A { @foo a(): string {} }", false);
        assert.equal(result?.ts, "class A { @foo a(): string {} }")
        assert.equal(result?.js, `class A {
    @foo
    a() { }
}
`);
    });

    it("should return null when there's a syntax error", () => {
        const result = processTs("function a() {", false);
        assert.equal(result, null);
    });
    it("should preserve module statements", () => {
        const result = processTs("import a from 'a'; import a = require('a'); export = 'b'; export const b = 'b'", false);
        assert.equal(result?.js, `import a from 'a';
const a = require("a");
module.exports = 'b';
export const b = 'b';
`);
    });

    it("should detect script type", () => {
        const result = processTs("console.log(1)", false);
        assert.equal(result?.kind, 'script');
    })
    it("should detect module type", () => {
        const result = processTs("export const a = 1", false);
        assert.equal(result?.kind, 'module');
    })
    it("should remove enums and namespaces", () => {
        const result = processTs("'你好'\nenum a {}\nnamespace b{}", true);
        assert.equal(result?.ts, "'你好';;");
    })
    it("should preserve declared enums and namespaces", () => {
        const result = processTs("declare enum a {}\ndeclare namespace b{}", true);
        assert.equal(result?.ts, "declare enum a {}\ndeclare namespace b{}");
    })
    it("should be able to strip parameters with modifiers", () => {
        const result = processTs("class A { constructor(private a, b) {} }", true, true);
        assert.equal(result?.ts, "class A { constructor( a, b) {} }");
    })
});

// describe("formatJs", () => {
//     it("should format module js", () => {
//         const result = formatJs("await 1+1");
//         assert.equal(result, "await 1 + 1;\n");
//     })
//     it("should reject invalid js syntax", () => {
//         const result = formatJs("let a: string");
//         assert.equal(result, null);
//     })
// });
