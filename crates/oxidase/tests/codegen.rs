mod utils;

use utils::check_transpile;

#[test]
fn prop_param_empty_constructor() {
    check_transpile("class A { constructor(private a, readonly b) { } }", "class A { a; b; constructor( a,  b) { this.a = a; this.b = b; } }");
}

#[test]
fn prop_param_non_empty_constructor() {
    check_transpile("class A { constructor(private a, readonly b) { foo<T>(); } }", "class A { a; b; constructor( a,  b) { this.a = a; this.b = b; foo(); } }");
}

#[test]
fn prop_param_super() {
    check_transpile("class A { constructor(private a, readonly b) { foo<T>(); super() } }", "class A { a; b; constructor( a,  b) { foo(); super(), this.a = a; this.b = b; } }");
}

#[test]
fn prop_param_super_semicolon() {
    check_transpile("class A { constructor(private a, readonly b) { foo<T>(); super(); } }", "class A { a; b; constructor( a,  b) { foo(); super(), this.a = a; this.b = b; } }");
}

#[test]
fn declare_class_with_codegen_inside() {
    check_transpile("declare class A { constructor(private a, readonly b) { } }", "");
}
