mod utils;

use utils::check_transpile;

#[test]
fn strip_stmt() {
    check_transpile("a;interface X{} b;declare function x();c", "a; b;c");
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
fn strip_var_definite() {
    check_transpile("var a! = null", "var a = null");
}
