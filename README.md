# Oxidase 

**Transpiles TypeScript at the Speed of Parsing** 

[![npm Badge](https://img.shields.io/npm/v/oxidase.svg)](https://www.npmjs.com/package/oxidase)

- 🧽 Strips types without source maps, inspired by [ts-blank-space](https://bloomberg.github.io/ts-blank-space/).
- 💪 Transforms enums, namespaces, and parameter properties.
- ⚡️  As fast as just parsing the input into AST nodes (see [Benchmark](#benchmark)).

[Playground](https://branchseer.github.io/oxidase/)

## Installation

`npm install -D oxidase`

## Usages

### Node.js Loader

```
$ node --import oxidase/register your-ts-file.ts
```

### JavaScript API

```js
import { transpile } from 'oxidase';

transpile("let a: number = 1"); // returns 'let a         = 1'
```


## Type-Stripping Transpilers


Type-stripping is a clever technique to transpile TypeScript by erasing the types and replacing them with whitespace. This approach preserves positions of all JavaScript code in the output, eliminating the need for sourcemaps.

The idea originated from [ts-blank-space](https://bloomberg.github.io/ts-blank-space/), and was later implemented by [swc_fast_ts_strip](https://github.com/swc-project/swc/tree/main/crates/swc_fast_ts_strip) in Rust, as the default built-in TypeScript transpiler in Node.js v22.6.0+.

Type-stripping inherently lacks support for non-erasable syntaxes such as enums, namespaces, and parameter properties. Oxidase aims to be a faster alternative while supporting these syntaxes.

## Enums, Namespaces and Parameter Properties

Oxidase carefully chooses where to insert code to preserve original code positions in most cases.

Input:
```
enum Foo {
    A = 1,
    B = A + 2,
}
```

Output:

```
var  Foo;(function(Foo){ {
  A = 1;var A;this[this.A=A]='A';
  B = A + 2;var B;this[this.B=B]='B';
}}).call(Foo||(Foo={}),Foo);
```

Notice that  `Foo`, `A = 1`, and `B = A + 2` are unchanged, and their positions are preserved.

In rare cases where enum members are in the same line: 


```ts
enum Foo { A = 1, B = A + 2 }
```

their columns positions are not preserved, whereas their line positions, and positions of code after the enum, are still preserved.

<details>

<summary>Why not generate sourcemap for cases like this?</summary>

Ideally the columns positions can be conveyed by a few entries in a sourcemap, but currently we have to generate at least one mapping per-line ([the chromium issue](https://issues.chromium.org/issues/364917746)) in a sourcemap.

That means the sourcemap size would be linear to the total line count. To me the cost (of both implementation and performance) is too big for such small limitation. Let's see if 
[Range Mappings](https://github.com/tc39/ecma426/pull/169) can offer a potential solution.

That said, PRs are always welcome if anyone is interested in implementing it.

</details>

## Performance

Here are some implementation details that make Oxidase fast. Skip to the [Benchmark](#benchmark) section if you just want to see the results.

<details>

<summary>No AST Allocations</summary>

Oxidase uses a [modified version of oxc_parser](https://github.com/branchseer/oxc/tree/ast_alloc), which does not allocate AST but exposes a [SAX](https://en.wikipedia.org/wiki/Simple_API_for_XML)-style API that streams AST nodes to a [handler](https://github.com/branchseer/oxc/blob/ast_alloc/crates/oxc_ast/src/generated/handle.rs). Oxidase collects position information in the handler as the parsing goes on.

</details>

<details>

<summary>In-Place Character Replacements</summary>

For sources with only erasable syntax, all positions of JavaScript code are preserved. Oxidase takes advantage of this and performs character replacements **directly in the input buffer**, avoiding writing the whole output.

Take `let a: string = ''` as an example. Oxidase would replace `: string` with the same amount of whitespaces in the original source buffer, **leaving `let a` and ` = ''` intact**.

> This optimization requires a mutable buffer of the input source. Since we always do copies when converting strings from JavaScript (UTF16) to Rust (UTF8), this shouldn't be a problem in practice.


</details>


<details>

<summary>Fast-Skipping Ambient Declarations</summary>

Ambient declarations (e.g., `interface`, `declare module`) are processed by **skipping tokens until the matching `}` appears**, not full parsing.


For example, when processing `interface Foo { a: { b: string }, c: string }`, Oxidase sees it as `interface Foo { ... { ... } ... }`.

Not only does it improve performance on large declarations, but it also provides some forward compatibility: Oxidase can happily process and erase unrecognized syntaxes inside a declaration:

```ts
interface A {
    this % is $ not ! valid ~ typescript for now, but {who} knows about the future
}
```

> Not all erasable syntaxes can be processed this way. Consider `A<{ a: 1 & 2 }>(0)` and `A<{ a: 1 + 2 }>(0)`, the first one is a function call with type instantiation which should be erased; the second one is a comparison expression between `A`, `{ a: 1 + 2 }` and `(0)`. Oxiase must rigourously parse what's between `{` and `}` to differentiate the two cases.


</details>

## Benchmark

[crates/bench](./crates/bench) compares the speed and memory usage of Oxidase with

- The original oxc_parser that allocates AST nodes. (just parsing, no transformation).
- [swc_fast_ts_strip](https://github.com/swc-project/swc/tree/main/crates/swc_fast_ts_strip), the built-in TypeScript transpiler in Node.js v22.6.0+

|   | Oxidase  | oxc_parser | swc_fast_ts_strip |
| - | - | - | - |
| Time | 1 | 1x[^1]  | 4x |
| Memory  | 1 | 2x ~ 11x[^2]  | 30x |

Check the [action run](https://github.com/branchseer/oxidase/actions/runs/13226071637/) for the detailed results. 

[^1]: Oxidase is slightly slower that oxc_parser on Linux x64, and slightly faster on macOS arm64.

[^2]: Depends on whether there are non-erasable syntaxes (enums, namespaces, etc.).
