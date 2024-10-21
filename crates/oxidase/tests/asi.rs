mod utils;

use utils::check_transpile;

#[test]
fn stmt_asi() {
    check_transpile("a\ninterface A {}\n/a/", "a\n;\n/a/")
}
#[test]
fn stmt_asi_first() {
    check_transpile("a; () => {\ninterface A {}\n/a/}", "a; () => {\n\n/a/}")
}

#[test]
fn stmt_postfix_asi() {
    check_transpile("var a = x as X\n(1)", "var a = x;\n(1)")
}
