use std::ops::Deref;

use swc_ecma_ast::{
    AssignTarget, BindingIdent, ClassMember, Expr, Lit, Module, ModuleItem, ParenExpr, Program,
    Script, SimpleAssignTarget, Stmt,
};
use swc_ecma_visit::{VisitMut, VisitMutWith, VisitWith};

pub struct EmptyStatementRemover;

impl VisitMut for EmptyStatementRemover {
    fn visit_mut_class_members(&mut self, members: &mut Vec<ClassMember>) {
        members.retain(|member: &ClassMember| !matches!(member, ClassMember::Empty(..)));
        members.visit_mut_children_with(self);
    }
    fn visit_mut_module_items(&mut self, items: &mut Vec<ModuleItem>) {
        items.retain(|item| !matches!(item, ModuleItem::Stmt(Stmt::Empty(..))));
        items.visit_mut_children_with(self);
    }
    fn visit_mut_stmts(&mut self, stmts: &mut Vec<Stmt>) {
        stmts.retain(|stmt| !matches!(stmt, Stmt::Empty(..)));
        stmts.visit_mut_children_with(self);
    }
    fn visit_mut_lit(&mut self, lit: &mut Lit) {
        if let Lit::Str(str_lit) = lit {
            // Make printer generate consistent quote type instead of relying on the input
            str_lit.raw = None;
        }
        lit.visit_mut_children_with(self);
    }
    fn visit_mut_expr(&mut self, expr: &mut Expr) {
        if let Expr::Paren(paren_expr) = expr {
            let inner_expr = paren_expr.expr.deref();
            if matches!(
                inner_expr,
                Expr::Lit(_)
                    | Expr::Paren(_)
                    | Expr::Ident(_)
                    | Expr::Member(_)
                    | Expr::Call(_)
                    | Expr::New(_)
                    | Expr::This(_)
                    | Expr::Array(_)
                    | Expr::SuperProp(_)
            ) {
                // `(a)` => `a`
                *expr = inner_expr.clone();
                expr.visit_mut_with(self);
                return;
            }
        }
        expr.visit_mut_children_with(self);
    }
    fn visit_mut_simple_assign_target(&mut self, node: &mut SimpleAssignTarget) {
        if let SimpleAssignTarget::Paren(paren_expr) = node {
            let inner_expr = paren_expr.expr.deref();
            match inner_expr {
                Expr::Ident(ident) => {
                    // `(id) = 1` => `id = 1`
                    *node = SimpleAssignTarget::Ident(BindingIdent {
                        id: ident.clone(),
                        type_ann: None,
                    });
                }
                Expr::Member(member) => {
                    // `(a.b) = 1` => `a.b = 1`
                    *node = SimpleAssignTarget::Member(member.clone());
                }
                Expr::Paren(paren) => {
                    // `(...) = 1` => `... = 1`
                    *node = SimpleAssignTarget::Paren(paren.clone());
                    node.visit_mut_with(self);
                    return;
                }
                _ => {}
            }
        }
        node.visit_mut_children_with(self);
    }
}

pub fn remove_empty_statements(node: &mut Program) {
    node.visit_mut_with(&mut EmptyStatementRemover);
}
