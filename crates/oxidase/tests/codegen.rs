mod utils;

use oxidase::{transpile, Allocator, SourceType, String};
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
    check_transpile("class A { constructor(private a, readonly b) { foo<T>(); super() } }", "class A { a; b; constructor( a,  b) { foo(); super(); this.a = a; this.b = b; } }");
}

#[test]
fn prop_param_super_semicolon() {
    check_transpile("class A { constructor(private a, readonly b) { foo<T>(); super(); } }", "class A { a; b; constructor( a,  b) { foo(); super(); this.a = a; this.b = b; } }");
}

#[test]
fn declare_class_with_codegen_inside() {
    check_transpile("declare class A { constructor(private a, readonly b) { } }", "");
}

#[test]
fn prop_param_with_type() {
    check_transpile("class X { a; constructor(a: number) { this.a = a; } }", "class X { a; constructor(a) { this.a = a; } }");
}

#[test]
fn prop_param_in_function_type() {
    check_transpile(r#"class C1 { constructor(a: (public b) => void) {} }"#, "class C1 { constructor(a) {} }");
}



#[test]
fn prop_param_optional() {
    check_transpile(r#"class A { constructor(public a?) {} }"#, "class A { a; constructor( a) { this.a = a;} }");
}


#[test]
fn prop_param_init() {
    check_transpile(r#"class A { constructor(public a: string = 1) {} }"#, "class A { a; constructor( a = 1) { this.a = a;} }");
}

#[test]
fn export_import_eq() {
    check_transpile(r#"class A { constructor(public a: string = 1) {} }"#, "class A { a; constructor( a = 1) { this.a = a;} }");
}

#[test]
fn prop_param_prologue() {
    check_transpile(r#"class B { constructor(public arg) { "a"; alert(1) } }"#, r#"class B { arg; constructor( arg) { "a"; this.arg = arg; alert(1) } }"#);
}

const s: &str = r#"a = <A><B>1"#;

#[test]
fn dbg() {
    let allocator = Allocator::default();
    let mut source = String::from_str_in(s, &allocator);
    let ret = transpile(
        &allocator,
            SourceType::ts(),
            &mut source,
    );
    println!("{}", source.as_str());
}
