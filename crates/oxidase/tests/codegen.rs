mod utils;

use utils::check_transpile;

#[test]
fn prop_param_empty_constructor() {
    check_transpile("class A { constructor(private a, readonly b) { } }", "class A { constructor(a, b) { this.a = a; this.b = b; } }");
}

#[test]
fn prop_param_non_empty_constructor() {
    check_transpile("class A { constructor(private a, readonly b) { foo<T>(); } }", "class A { constructor(a, b) { this.a = a; this.b = b; foo(); } }");
}

#[test]
fn prop_param_after_super_call() {
    check_transpile("class A { constructor(private a) { super(); } }", "class A { constructor(a) { } }");
}
