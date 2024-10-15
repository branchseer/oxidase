mod utils;

use utils::check_transpile;

#[test]
fn strip_stmt() {
    check_transpile("a;interface X{} b;declare function x();c", "a; b;c");
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
fn strip_type_assertions() {
    check_transpile("1 as A;2 satisfies B;", "1;2;");
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
    check_transpile("function a(this: A,) {}", "function a() {}");
}
