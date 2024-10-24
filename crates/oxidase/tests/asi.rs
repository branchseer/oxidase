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

#[test]
fn stmt_inner_postfix_asi() {
    check_transpile("var a = () => x as X\n(1)", "var a = () => x;\n(1)")
}

#[test]
fn arrow_expression_no_asi() {
    check_transpile("f(() => 1)", "f(() => 1)")
}


#[test]
fn if_asi1() {
    check_transpile("if (1) type A = string", "if (1) ;")
}

#[test]
fn if_asi2() {
    check_transpile("if (1) type A = string\nelse { type A = string }", "if (1) ;\nelse {  }")
}

#[test]
fn if_asi3() {
    check_transpile("if (1) type A = string\nelse type A = string", "if (1) ;\nelse ;")
}
#[test]
fn if_asi4() {
    check_transpile("if (1) {} else type A = string", "if (1) {} else ;")
}

#[test]
fn while_asi() {
    check_transpile("while (1) type A = string", "while (1) ;")
}

#[test]
fn for_asi() {
    check_transpile("for (;;) type A = string", "for (;;) ;")
}

#[test]
fn for_in_asi() {
    check_transpile("for (var a in {}) type A = string", "for (var a in {}) ;")
}

#[test]
fn for_of_asi() {
    check_transpile("for (var a of []) type A = string", "for (var a of []) ;")
}



#[test]
fn class_element_asi() {
    check_transpile(r#"
class A {
    a = 1
    abstract b
    ['c'](){}
}"#, r#"
class A {
    a = 1
    ;
    ['c'](){}
}"#)
}

#[test]
fn modifier_computed_prop_asi() {
    check_transpile(r#"
class A {
    a = 1
    private ['c'](){}
}"#, r#"
class A {
    a = 1
    ;['c'](){}
}"#)
}

#[test]
fn modifier_generator_asi() {
    check_transpile(r#"
class A {
    a = 1
    private *c(){}
}"#, r#"
class A {
    a = 1
    ;*c(){}
}"#)
}
