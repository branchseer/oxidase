use googletest::{prelude::*, test};

use rquickjs::{Ctx, Function, Module};
use serde_json::json;
fn check_transpile(source: &str, expected_out: &str) {
    todo!()
    // let mut out = String::new();
    // transpile(ModuleKind::Module, source, &mut out, &mut Strict::default()).unwrap();
    // expect_eq!(out, expected_out);
}
fn check_transpile_identical(source: &str) {
    check_transpile(source, source);
}

#[test]
fn empty_source() {
    check_transpile_identical("");
}
#[test]
fn regex_at_start() {
    check_transpile_identical("/a}/");
}

#[test]
fn var_decl_types() {
    check_transpile(
        "const foo: string = 1, bar: number = 2, { baz }: baz = {};",
        "const foo = 1, bar = 2, { baz } = {};",
    );
}

#[test]
fn non_type_annotation_colons() {
    check_transpile_identical("const { foo: bar, a: b } = { foo: 1, a: b };");
    check_transpile_identical("true ? 1 : 2");
    check_transpile_identical("switch (1) { case (1): break; }");
    check_transpile_identical("label: 1\nfunction foo() { label: 1 }");
}

#[test]
fn non_type_annotation() {
    check_transpile_identical("const { z, a, b } = { z: 0,{ a: 0, b: 0} }");
}

#[test]
fn function_arg_ret_types() {
    check_transpile(
        "function foo(a: string, b: number): foo {}; function bar<T>(a: T): zz {}, function* baz<T>(a: T) {}",
        "function foo(a, b) {}; function bar(a) {}, function* baz(a) {}"
    );
}

#[test]
fn retokenize() {
    check_transpile_identical("function foo() {}\n/a}/.test('')");
}

// #[test]
// fn retokenize_after_type() {
//     check_transpile("type X = string\n/a}/.test('')", ";/a}/.test('')");
// }

#[test]
fn type_assertions() {
    check_transpile("var as = 1\nas = as as as", "var as = 1\nas = as");
    check_transpile("var as = 1;as = as as as", "var as = 1;as = as");
}

#[test]
fn as_identifer() {
    check_transpile_identical("let as = 1\n");
}
#[test]
fn as_field() {
    check_transpile_identical("a.as\n1");
}
#[test]
fn namespace_as_as() {
    check_transpile_identical("import * as as from 'a'");
}

#[test]
fn type_assertions_const() {
    check_transpile("1 as const", "1");
}

#[test]
fn re_lex_after_type_assertions() {
    check_transpile("`${1 as {}}}`", "`${1}}`");
}

#[test]
fn re_lex_after_type() {
    check_transpile("`${1 as number}`", "`${1}`");
}

#[test]
fn generics_decl() {
    check_transpile(
        "class foo<T> {}; const bar = class<T> {}; function baz<T extends Foo<Bar>>(a: T) {}",
        "class foo {}; const bar = class {}; function baz(a) {}",
    );
    check_transpile(
        "class foo { a = foo<T>(a)\nfoo<T>(a) {}}",
        "class foo { a = foo(a)\nfoo(a) {}}",
    );
}

#[test]
fn function_signature_overloading() {
    check_transpile(
        "function a()\nfunction a(): void\nfunction a();function a<T>(a: T): void\nfunction a() {}",
        "\n\n\nfunction a() {}",
    );
}
#[test]
fn function_signature_overloading_export() {
    check_transpile(
        "let x = 1\nexport async function* a()\nfunction a() {}",
        "let x = 1;\n\nfunction a() {}",
    );
}

// #[test]
// fn for_debug() {
//     check_transpile_identical(
//         r#"//// [tests/cases/conformance/externalModules/typeOnly/importDefaultNamedType.ts] ////

// //// [a.ts]
// export default class A {}

// //// [b.ts]
// import type from './a';

// //// [a.js]
// "use strict";
// Object.defineProperty(exports, "__esModule", { value: true });
// var A = /** @class */ (function () {
//     function A() {
//     }
//     return A;
// }());
// exports.default = A;
// //// [b.js]
// "use strict";
// Object.defineProperty(exports, "__esModule", { value: true });
// "#,
//     );
// }

#[test]
fn method_signature_overloading_simple() {
    check_transpile(
        "class A { ['a']()\n['a'](a: any){ } }",
        "class A { \n['a'](a){ } }",
    );
}

#[test]
fn method_signature_overloading_multiple() {
    check_transpile(
        r#"class A { foo = 1 +
a()
a()
a<T>(): void
a() {}
}"#,
        r#"class A { foo = 1 +
a();


a() {}
}"#,
    );
}

#[test]
fn method_signature_overloading_modifier() {
    check_transpile(
        r#"
class A { foo = bar
    override a(): void
    a() { }
}"#,
        r#"
class A { foo = bar;
    
    a() { }
}"#,
    );
}

#[test]
fn method_signature_overloading_modifier_semicolon() {
    check_transpile(
        r#"
class A { foo() { }
    override a(): void
    a() { }
}"#,
        r#"
class A { foo() { };
    
    a() { }
}"#,
    );
}

#[test]
fn index_signature() {
    check_transpile(
        r#"
class A {
    readonly [a: string]: string
    a() { }
}"#,
        r#"
class A {
    
    a() { }
}"#,
    );
}

#[test]
fn index_signature_static() {
    check_transpile(
        r#"
class A {
    static [a: string]: string
}"#,
        r#"
class A {
    
}"#,
    );
}
#[test]
fn index_signature_static_readonly() {
    check_transpile(
        r#"
class A {
    static readonly [a: string]: string
}"#,
        r#"
class A {
    
}"#,
    );
}
#[test]
fn index_signature_static_readonly_field() {
    check_transpile(
        r#"
class A {
    static readonly
    readonly [a: string]: string
}"#,
        r#"
class A {
    static readonly;
    
}"#,
    );
}

#[test]
fn this_param_in_function() {
    check_transpile("function a(this) {}", "function a() {}");
}

#[test]
fn this_param_with_type_in_function() {
    check_transpile("function a(this: string) {}", "function a() {}");
}

#[test]
fn this_param_function_generator() {
    check_transpile("function* a(this: string) {}", "function* a() {}");
}

#[test]
fn this_param_function_no_name() {
    check_transpile("function(this: string) {}", "function() {}");
}
#[test]
fn this_param_class_method() {
    check_transpile("class a { foo(this) {} }", "class a { foo() {} }");
}

#[test]
fn this_param_class_getter() {
    check_transpile("class a { get foo(this) {} }", "class a { get foo() {} }");
}

#[test]
fn this_param_object_method() {
    check_transpile("a = { foo(this) {} }", "a = { foo() {} }");
}
#[test]
fn this_param_object_getter() {
    check_transpile("a = { get foo(this) {} }", "a = { get foo() {} }");
}

#[test]
fn index_signature_semicolon() {
    check_transpile(
        r#"
class A {
    readonly [a: string]: string;
    a() { }
}"#,
        r#"
class A {
    
    a() { }
}"#,
    );
}
#[test]
fn index_signature_asi() {
    check_transpile(
        r#"
class A {
    foo
    readonly [a: string]: string;
    a() { }
}"#,
        r#"
class A {
    foo;
    
    a() { }
}"#,
    );
}

#[test]
fn class_modifier_after_static() {
    check_transpile(
        "class A { static readonly a = 1 }",
        "class A { static a = 1 }",
    );
}

#[test]
fn class_modifier_after_keyword_identifier() {
    check_transpile(
        r#"class Z {
    static new
    private a() {}
}"#,
        r#"class Z {
    static new
    a() {}
}"#,
    )
}

#[test]
fn class_modifier_asi() {
    check_transpile(
        r#"
class A {
    foo = bar
    readonly ['a'] = 2
}"#,
        r#"
class A {
    foo = bar;
    ['a'] = 2
}"#,
    );
}

#[test]
fn class_method_modifier() {
    check_transpile("class A { public foo() {} }", "class A { foo() {} }");
}

#[test]
fn class_modifiers() {
    check_transpile(
        r#"
        abstract
        class Foo { }
        abstract class Bar {
            private a
            private
            private a
            abstract() { }
            private override abstract() { }
            static readonly private
            override static readonly private a
            override static readonly private
            private get a() {}
            get private() { }
            private a
            private = 2
            private: any
            static private
            private static
            private;
            a;
            private = {}
            private ['a']() {}
            x = 1 +
            private ['a']()
            x = 1
            + private ['a']()
            private
        }
        "#,
        r#"
        abstract
        class Foo { }
        class Bar {
            a
            private
            a
            abstract() { }
            abstract() { }
            static private
            static a
            static private
            get a() {}
            get private() { }
            a
            private = 2
            private
            static private
            static
            private;
            a;
            private = {};
            ['a']() {}
            x = 1 +
            private ['a']()
            x = 1
            + private ['a']()
            private
        }
        "#,
    )
}

#[test]
fn bang_non_null_assertion() {
    check_transpile("let a!; a = a! + !a", "let a; a = a + !a");
    check_transpile("a!()\n!a", "a()\n!a");
    check_transpile("class Foo { foo!: any }", "class Foo { foo }");
}

#[test]
fn bang_after_arrow() {
    check_transpile_identical("() => !a");
}
#[test]
fn bang_after_comma() {
    check_transpile_identical("a, !a");
}
#[test]
fn bang_class_computed_prop() {
    check_transpile("class Foo { ['a']!: any }", "class Foo { ['a'] }");
}

#[test]
fn question_optional_mark() {
    check_transpile(
        "function a(a?, b?: any, c?) {} class Foo { foo?\nbar?(){}\na=1?b:c }",
        "function a(a, b, c) {} class Foo { foo\nbar(){}\na=1?b:c }",
    );
}

#[test]
fn class_name_implements() {
    check_transpile("class A implements B {}", "class A {}");
}
#[test]
fn class_implements() {
    check_transpile("class implements B {}", "class {}");
}
#[test]
fn class_extends_implements() {
    check_transpile("class extends A implements B {}", "class extends A {}");
}

fn check_enum_value(
    export: bool,
    enum_source: &str,
    enum_name: &str,
    expected_json: serde_json::Value,
) {
    use rquickjs::context::EvalOptions;
    use rquickjs::{Coerced, Context, Error, Object, Runtime};

    fn stringify_err<T>(
        ctx: &Ctx<'_>,
        res: core::result::Result<T, Error>,
    ) -> core::result::Result<T, String> {
        match res {
            Ok(json) => Ok(json),
            Err(Error::Exception) => {
                let message = ctx.catch().get::<Coerced<String>>().unwrap().0;
                let stack = ctx
                    .catch()
                    .get::<rquickjs::Exception>()
                    .ok()
                    .and_then(|exception| exception.stack());
                Err(format!(
                    "Message: {}. Stack: {}",
                    message,
                    stack.unwrap_or(String::from("N/A")),
                ))
            }
            Err(other) => panic!("Unexpected quickjs error: {:?}", other),
        }
    }

    let mut out = String::new();
    // transpile(
    //     if export {
    //         ModuleKind::Module
    //     } else {
    //         ModuleKind::Script
    //     },
    //     enum_source,
    //     &mut out,
    //     &mut Strict::default(),
    // )
    // .unwrap();
    let rt = Runtime::new().unwrap();
    let context = Context::full(&rt).unwrap(); // TODO: check why Context::base aborts
    let actual_json = context.with(|ctx| {
        if export {
            let module = stringify_err(
                &ctx,
                Module::declare(ctx.clone(), "enum_test_module.js", out.to_string()),
            )?;
            let (module, _) = stringify_err(&ctx, Module::eval(module))?;
            let enum_object = stringify_err(&ctx, module.get::<_, Object>(enum_name))?;
            let json_stringify =
                stringify_err(&ctx, ctx.eval::<Function, _>("obj => JSON.stringify(obj)"))?;
            let json = stringify_err(&ctx, json_stringify.call::<_, String>((enum_object,)))?;
            Ok(json)
        } else {
            let source = format!("{}\nJSON.stringify({}, null, 2)", &out, enum_name);
            let mut eval_options = EvalOptions::default();
            eval_options.strict = false;

            stringify_err(
                &ctx,
                ctx.eval_with_options::<String, _>(source, eval_options),
            )
        }
    });
    match actual_json {
        Ok(actual_json) => {
            expect_eq!(
                serde_json::from_str::<serde_json::Value>(&actual_json).unwrap(),
                expected_json,
                "---- Transpiled Enum Source ----\n{}\n--------\n",
                out
            );
        }
        Err(js_exception) => {
            add_failure!("---source execution failed---\n{}\n{}\n", out, js_exception)
        }
    }
}

#[test]
fn ts_enum() {
    check_enum_value(false, "enum A { A }", "A", json!({ "A": 0, "0": "A" }));
    check_enum_value(
        false,
        "enum A { A, B = A + 3, }",
        "A",
        json!({ "A": 0, "0": "A", "B": 3, "3": "B" }),
    );
    check_enum_value(
        false,
        r#"enum A { \u{0065}, B = e + 3, }"#,
        "A",
        json!({ "e": 0, "0": "e", "B": 3, "3": "B" }),
    );

    check_enum_value(false, "enum A { }", "A", json!({}));

    check_enum_value(
        false,
        "enum A { A, B = 21, C }",
        "A",
        json!({ "A": 0, "B": 21, "0": "A", "21": "B", "C": 22, "22": "C" }),
    );

    check_enum_value(
        false,
        "enum A { A, B = 21, C = 22 }",
        "A",
        json!({ "A": 0, "B": 21, "0": "A", "21": "B", "C": 22, "22": "C" }),
    );

    check_enum_value(
        false,
        r#"enum A { "X" = 3, B, C = X - 3, }"#,
        "A",
        json!({ "X": 3, "3": "X", "B": 4, "4": "B", "C": 0, "0": "C" }),
    );
}

#[test]
fn ts_enum_unicode_escaped_name() {
    check_enum_value(
        false,
        r#"enum \u{0041} { Foo }"#,
        "A",
        json!({ "Foo": 0, "0": "Foo" }),
    );
}

#[test]
fn ts_enum_type_annotation_in_value_trailing_semicolon() {
    check_enum_value(
        false,
        r#"enum A { "X" = 1 as number, }"#,
        "A",
        json!({ "X": 1, "1": "X", }),
    );
}
#[test]
fn ts_enum_type_annotation_in_value() {
    check_enum_value(
        false,
        r#"enum A { "X" = 1 as number }"#,
        "A",
        json!({ "X": 1, "1": "X", }),
    );
}

#[test]
fn ts_enum_keywords() {
    check_enum_value(
        false,
        "enum A { let, await = let + 1, yield = await + 1, }",
        "A",
        json!({ "let": 0, "0": "let", "await": 1, "1": "await", "yield": 2, "2": "yield" }),
    );
    check_enum_value(
        false,
        "'use strict'; enum A { let, await, yield }",
        "A",
        json!({ "let": 0, "0": "let", "await": 1, "1": "await", "yield": 2, "2": "yield" }),
    );
    check_enum_value(
        false,
        "var A; (async function() { enum B { let, await = let + 1, yield = 2 }; A = B })();",
        "A",
        json!({ "let": 0, "0": "let", "await": 1, "1": "await", "yield": 2, "2": "yield" }),
    );
}

#[test]
fn ts_enum_nested() {
    check_enum_value(
        false,
        "enum A {
    Foo = (() => {
        enum A {
            Foo,
        }
        return A.Foo + 1
    })()
}",
        "A",
        json!({ "Foo": 1, "1": "Foo" }),
    );
}

#[test]
fn ts_enum_export() {
    check_enum_value(
        true,
        "export enum A { Foo }",
        "A",
        json!({ "Foo": 0, "0": "Foo" }),
    );
}

#[test]
fn ts_enum_export_const() {
    check_enum_value(
        true,
        "export const enum A { Foo }",
        "A",
        json!({ "Foo": 0, "0": "Foo" }),
    );
}

#[test]
fn ts_enum_merge_basic() {
    check_enum_value(
        false,
        "enum Foo { A }\nenum Foo { B = A + 2, C = Foo.A + 3} ",
        "Foo",
        json!({ "A": 0, "0": "A", "B": 2, "2": "B", "C": 3, "3": "C" }),
    );
}

#[test]
fn ts_enum_merge_const() {
    check_enum_value(
        false,
        "const enum Foo { A }\nconst enum Foo { B = A + 2, C = Foo.A + 3} ",
        "Foo",
        json!({ "A": 0, "0": "A", "B": 2, "2": "B", "C": 3, "3": "C" }),
    );
}
#[test]
fn ts_enum_merge_export() {
    check_enum_value(
        true,
        "export enum Foo { A }\nexport enum Foo { B = A + 2, C = Foo.A + 3} ",
        "Foo",
        json!({ "A": 0, "0": "A", "B": 2, "2": "B", "C": 3, "3": "C" }),
    );
}

#[test]
fn ts_enum_merge_shadow() {
    check_enum_value(
        false,
        "enum A { Foo = 1, A }\nenum A { B = A + 3} ",
        "A",
        json!({ "Foo": 1, "1": "Foo", "A": 2, "B": 5, "2": "A", "5": "B" }),
    );
}

#[test]
fn ts_type_arguments_basic() {
    check_transpile("foo<string>()", "foo()");
}

#[test]
fn ts_type_arguments_invalid() {
    check_transpile_identical("foo<string>1");
}

#[test]
fn ts_type_arguments_asi() {
    check_transpile("foo<string>\n//comment\nfunction a()", "foo;\n//comment\n");
}

#[test]
fn ts_type_arguments_before_type_assertion() {
    check_transpile("foo<string> as string", "foo");
}

#[test]
fn ts_type_alias() {
    check_transpile("foo\ntype A = string\nbar", "foo;\n\nbar");
}

#[test]
fn ts_export_type_alias() {
    check_transpile("foo\nexport type A = string\nbar", "foo;\n\nbar");
}
#[test]
fn ts_type_alias_with_semicolon() {
    check_transpile("foo\ntype A = string;\nbar", "foo;\n\nbar");
}

#[test]
fn ts_type_alias_generics() {
    check_transpile("foo\ntype A<T = U<X>> = string\nbar", "foo;\n\nbar");
}

#[test]
fn ts_import_type() {
    check_transpile("foo\nimport type { a } from 'a'\nbar", "foo;\n\nbar");
}

#[test]
fn import_name_type() {
    check_transpile_identical("foo\nimport type from 'a'\nbar");
}
#[test]
fn import_type_type() {
    check_transpile("foo\nimport type type from 'a'\nbar", "foo;\n\nbar");
}

#[test]
fn import_type_comma() {
    check_transpile_identical("foo\nimport type, {} from 'a'\nbar");
}

#[test]
fn ts_import_type_default() {
    check_transpile("foo\nimport type a from 'a'\nbar", "foo;\n\nbar");
}
#[test]
fn ts_import_type_semicolon() {
    check_transpile("foo\nimport type a from 'a';\nbar", "foo;\n\nbar");
}
#[test]
fn ts_import_type_attributes() {
    check_transpile("foo\nimport type a from 'a' with {}\nbar", "foo;\n\nbar");
}

#[test]
fn ts_import_type_attributes_semicolon() {
    check_transpile("foo\nimport type a from 'a' with {};\nbar", "foo;\n\nbar");
}

#[test]
fn ts_export_type() {
    check_transpile(
        "foo\nexport type { a } from 'a' with {};\nbar",
        "foo;\n\nbar",
    );
}

#[test]
fn ts_export_type_without_source() {
    check_transpile("foo\nexport type { a }\nbar", "foo;\n\nbar");
}

#[test]
fn ts_export_type_with_source_with_semicolon() {
    check_transpile("foo\nexport type { a };\nbar", "foo;\n\nbar");
}
#[test]
fn ts_import_type_id_eq() {
    check_transpile("foo\nimport type Foo = a.b\nbar", "foo;\n\nbar");
}
#[test]
fn ts_import_type_from_eq() {
    check_transpile("foo\nimport type from = a.b\nbar", "foo;\n\nbar");
}

#[test]
fn ts_import_type_from() {
    check_transpile_identical("foo\nimport type from 'a'\nbar");
}

#[test]
fn ts_import_type_from_from() {
    check_transpile("foo\nimport type from from 'a'\nbar", "foo;\n\nbar");
}
#[test]
fn ts_import_type_type_from() {
    check_transpile("foo\nimport type type from 'a'\nbar", "foo;\n\nbar");
}

#[test]
fn ts_import_type_require() {
    check_transpile("foo\nimport type A = require('a')\nbar", "foo;\n\nbar");
}

#[test]
fn ts_import_type_require_id() {
    check_transpile("foo\nimport type A = require\nbar", "foo;\n\nbar");
}

#[test]
fn ts_import_type_id() {
    check_transpile("foo\nimport type A = A\nbar", "foo;\n\nbar");
}

#[test]
fn ts_import_type_id_dot() {
    check_transpile("foo\nimport type A = A.B.C\nbar", "foo;\n\nbar");
}
#[test]
fn ts_import_id_eq() {
    check_transpile(
        "foo\nimport A = require('a')\nbar",
        "foo\nconst A = require('a')\nbar",
    );
}
#[test]
fn ts_import_type_eq() {
    check_transpile(
        "foo\nimport type = require('a')\nbar",
        "foo\nconst type = require('a')\nbar",
    );
}
#[test]
fn export_as_namespace() {
    check_transpile("foo\nexport as namespace AA\nbar", "foo;\n\nbar");
}

#[test]
fn ts_export_eq() {
    check_transpile("export = 1", "module.exports = 1");
}
#[test]
fn ts_export_import() {
    check_transpile(
        "export import Foo = require('a')",
        "const Foo = exports.Foo = require('a')",
    );
}
#[test]
fn ts_export_import_type() {
    check_transpile(
        "foo\nexport import type Foo = require('a');\nbar",
        "foo;\n\nbar",
    );
}
#[test]
fn ts_dot_export_eq() {
    check_transpile_identical("a.export = 1");
}

#[test]
fn ts_import_type_type() {
    check_transpile("foo\nimport type type = A\nbar", "foo;\n\nbar");
}

#[test]
fn ts_interface() {
    check_transpile(
        "foo\ninterface Foo extends Bar<{}> { a }\nbar",
        "foo;\n\nbar",
    );
}

#[test]
fn ts_interface_semicolon() {
    check_transpile("foo\ninterface Foo { };\nbar", "foo;\n\nbar");
}
#[test]
fn ts_interface_export() {
    check_transpile("foo\nexport interface Foo {  }\nbar", "foo;\n\nbar");
}

#[test]
fn ts_interface_declare() {
    check_transpile("foo\ndeclare interface Foo {  }\nbar", "foo;\n\nbar");
}
#[test]
fn ts_interface_export_declare() {
    check_transpile("foo\nexport declare interface Foo {  }\nbar", "foo;\n\nbar");
}

#[test]
fn ts_declare_class() {
    check_transpile("foo\ndeclare class Z {}\nbar", "foo;\n\nbar");
}
#[test]
fn ts_declare_function() {
    check_transpile("foo\ndeclare function a()\nbar", "foo;\n\nbar");
}
#[test]
fn ts_declare_function_with_type() {
    check_transpile("foo\ndeclare function a(): string\nbar", "foo;\n\nbar");
}

#[test]
fn ts_declare_variable() {
    check_transpile("foo\ndeclare const a: string\nbar", "foo;\n\nbar");
}
#[test]
fn ts_declare_const_literal() {
    check_transpile("foo\ndeclare const a = 1\nbar", "foo;\n\nbar");
}

#[test]
fn ts_declare_const_id() {
    check_transpile("foo\ndeclare const a = b\nbar", "foo;\n\nbar");
}

#[test]
fn ts_declare_const_enum_dot_case() {
    check_transpile("foo\ndeclare const a = Foo.Bar\nbar", "foo;\n\nbar");
}
#[test]
fn ts_declare_const_enum_subscription() {
    check_transpile("foo\ndeclare const a = Foo['Bar']\nbar", "foo;\n\nbar");
}

#[test]
fn ts_declare_namespace() {
    check_transpile("foo\ndeclare namespace Z {}\nbar", "foo;\n\nbar");
}

#[test]
fn ts_declare_module() {
    check_transpile("foo\ndeclare module Z {}\nbar", "foo;\n\nbar");
}
#[test]
fn ts_enum_module() {
    check_transpile("foo\ndeclare enum Z {}\nbar", "foo;\n\nbar");
}

#[test]
fn ts_export_declare_class() {
    check_transpile("foo\nexport declare class Z {}\nbar", "foo;\n\nbar");
}

#[test]
fn declare_new_line() {
    check_transpile("foo\ndeclare\ntype A = string\nbar", "foo\ndeclare;\n\nbar");
}
#[test]
fn declare_type_alias() {
    check_transpile("foo\ndeclare type A = string\nbar", "foo;\n\nbar");
}

#[test]
fn ts_parameter_properties() {
    check_transpile(
        r#"class A { foo; constructor(private a, readonly b, private readonly c, d, readonly, readonly private) { } }"#,
        "class A { a; b; c; private; foo; constructor(a, b, c, d, readonly, private) { this.a = a; this.b = b; this.c = c; this.private = private; } }",
    );
}

#[test]
fn ts_parameter_properties_super() {
    check_transpile(
        r#"class A { foo; constructor(private a, readonly b, private readonly c, d, readonly, readonly private) { super() } }"#,
        "class A { a; b; c; private; foo; constructor(a, b, c, d, readonly, private) { super(), this.a = a, this.b = b, this.c = c, this.private = private } }",
    );
}
#[test]
fn ts_parameter_properties_super_as() {
    check_transpile(
        r#"class A { foo; constructor(private a, readonly b, private readonly c, d, readonly, readonly private) { super() as any } }"#,
        "class A { a; b; c; private; foo; constructor(a, b, c, d, readonly, private) { super(), this.a = a, this.b = b, this.c = c, this.private = private } }",
    );
}

#[test]
fn ts_parameter_properties_with_decl() {
    check_transpile(
        r#"
class A {
    constructor(a: string)
    constructor(private b) { }
}"#,
        r#"
class A {
    b; 
    constructor(b) { this.b = b; }
}"#,
    )
}
