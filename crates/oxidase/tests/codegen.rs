mod utils;

use utils::check_transpile;

#[test]
fn prop_param() {
    check_transpile("class A { constructor(private a) { super(); } }", "class A { constructor(a) { } }");
}

#[test]
fn prop_param_after_super_call() {
    check_transpile("class A { constructor(private a) { super(); } }", "class A { constructor(a) { } }");
}
