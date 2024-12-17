import { assertEquals } from "jsr:@std/assert";
import { describe, it } from "jsr:@std/testing/bdd";
import { transpile } from "./tsc.ts";

describe("transpile", () => {
    it('should format and transpile ts', () => {
        const result = transpile("class A { @foo a(): string {} }");
        assertEquals(result?.ts, `class A {
    @foo
    a(): string { }
}
`)

        assertEquals(result?.js, `class A {
    @foo
    a() { }
}
`);
    });

    it("should return null when there's a syntax error", () => {
        const result = transpile("function a() {");
        assertEquals(result, null);
    });
    it("should preserve module statements", () => {
        const result = transpile("import a from 'a'; import a = require('a'); export = 'b'; export const b = 'b'");
        assertEquals(result?.js, `import a from 'a';
const a = require("a");
module.exports = 'b';
export const b = 'b';
`);
    });

    it("should detect script type", () => {
        const result = transpile("console.log(1)");
        assertEquals(result?.kind, 'script');
    })
    it("should detect module type", () => {
        const result = transpile("export const a = 1");
        assertEquals(result?.kind, 'module');
    })
    it("should remove enums and namespaces", () => {
        const result = transpile("enum a {}\nnamespace b{}");
        assertEquals(result?.ts, '');
    })
    it("should preserve declared enums and namespaces", () => {
        const result = transpile("declare enum a {}\ndeclare namespace b{}");
        assertEquals(result?.ts, `declare enum a {
}
declare namespace b { }
`);
    })
});

// describe("formatJs", () => {
//     it("should format module js", () => {
//         const result = formatJs("await 1+1");
//         assertEquals(result, "await 1 + 1;\n");
//     })
//     it("should reject invalid js syntax", () => {
//         const result = formatJs("let a: string");
//         assertEquals(result, null);
//     })
// });
