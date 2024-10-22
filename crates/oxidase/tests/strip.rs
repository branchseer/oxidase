mod utils;

use utils::check_transpile;

#[test]
fn strip_stmt() {
    check_transpile("a;interface X{} b;declare function x();c", "a; b;c");
}


#[test]
fn strip_type_alias() {
    check_transpile("type A = string", "");
}


#[test]
fn strip_export_stripped_decl() {
    check_transpile("export type A = string", "");
}

#[test]
fn strip_stripped_decl_asi() {
    check_transpile("var a = 1\nexport type A = string\n/a/.test()", "var a = 1\n;\n/a/.test()");
}

#[test]
fn strip_import_eq() {
    check_transpile("import x = require('a')", "const x = require('a')");
}

#[test]
fn strip_export_eq() {
    check_transpile("var x = 1 as string; export = x as string;", "var x = 1; module.exports = x;");
}

#[test]
fn strip_declare_function_with_type_annotation() {
    check_transpile("declare function x(a: string): string", "");
}

#[test]
fn strip_rewind() {
    check_transpile("(foo<((a: string) => string), 1 + 1>(1))", "(foo<((a) => string), 1 + 1>(1))");
}

#[test]
fn strip_type_annotation() {
    check_transpile(
        "var a: number = 0; function b(x: string): void {}",
        "var a = 0; function b(x) {}",
    );
}

#[test]
fn strip_type_param() {
    check_transpile("a<string>(); function a<T>() {}", "a(); function a() {}");
}
#[test]
fn strip_type_force_cast() {
    check_transpile("1 as A;2 satisfies B;", "1;2;");
}

#[test]
fn strip_import_export_type_specifier() {

}

#[test]
fn strip_type_assertion() {
    check_transpile("var a = <number>1", "var a = 1");
}

#[test]
fn strip_class_abstract_basic() {
    check_transpile("abstract class A {}", "class A {}");
}

#[test]
fn strip_class_abstract_complex() {
    check_transpile(
        "@foo\nexport abstract class A {}",
        "@foo\nexport class A {}",
    );
}

#[test]
fn strip_this_paramater() {
    check_transpile("function a(this: A) {}", "function a() {}");
}


#[test]
fn strip_this_paramater_comma() {
    check_transpile("function a(this: A  ,) {}", "function a() {}");
}

#[test]
fn strip_property_modifiers() {
    check_transpile("class A { private readonly a; }", "class A { a; }");
}
#[test]
fn strip_property_optional() {
    check_transpile("class A { a? }", "class A { a }");
}

#[test]
fn strip_method_modifiers() {
    check_transpile("class A { private async abstract static a() { } }", "class A { static async a() { } }");
}

#[test]
fn strip_import_type_single() {
    check_transpile("import { type A } from 'a'", "import { } from 'a'");
}

#[test]
fn strip_import_type_trailing_comma() {
    check_transpile("import { A, type A, } from 'a'", "import { A, } from 'a'");
}
#[test]
fn strip_import_type_first() {
    check_transpile("import { type A, B } from 'a'", "import { B } from 'a'");
}

#[test]
fn strip_import_type_successive() {
    check_transpile("import { type B, type A, } from 'a'", "import { } from 'a'");
}

#[test]
fn strip_index_signature_with_modifiers() {
    check_transpile("class A { static [a: string]: string }", "class A {  }");
}

#[test]
fn strip_var_definite() {
    check_transpile("var a! = null", "var a = null");
}

#[test]
fn strip_expr_definite() {
    check_transpile("var a = null!;", "var a = null;");
}

#[test]
fn strip_class_implements() {
    check_transpile("class A implements B, C { }", "class A { }");
}

#[test]
fn strip_method_overload() {
    check_transpile("class A { a() }", "class A {  }");
}

#[test]
fn strip_abstract_prop() {
    check_transpile("class A { abstract a: string }", "class A {  }");
}

#[test]
fn strip_abstract_accessor() {
    check_transpile("class A { abstract accessor a: string }", "class A {  }");
}

#[test]
fn strip_accessor_modifier() {
    check_transpile("class Z { private accessor a; }", "class Z { accessor a; }");
}

#[test]
fn strip_getter_modifier() {
    check_transpile("class Z { private get a() {} }", "class Z { get a() {} }");
}


#[test]
fn strip_function_overload() {
    check_transpile("function a()", "");
}

#[test]
fn strip_export_decl() {
    check_transpile("export function a()", "");
}

#[test]
fn strip_export_default_decl() {
    check_transpile("export default function a()", "");
}

#[test]
fn wrap_object_with_type_assertion_after_arrow() {
    check_transpile("() => <Type>{ a: 1 }", "() => ({ a: 1 })");
}
