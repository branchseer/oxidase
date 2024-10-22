use oxidase::ModuleKind;
use serde::{Deserialize, Serialize};
use std::path::Path;
use swc_ecma_ast::{ClassMember, Module, ModuleItem, Program, Script, Stmt};
use swc_ecma_parser::Syntax;
use swc_ecma_visit::{VisitMut, VisitMutWith};

pub fn generated_folder_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("generated")
}

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
}

pub fn remove_empty_statements(node: &mut Program) {
    node.visit_mut_with(&mut EmptyStatementRemover);
}

pub fn ts_syntax() -> Syntax {
    Syntax::Typescript(swc_ecma_parser::TsSyntax {
        decorators: true,
        ..Default::default()
    })
}

pub fn test_repos_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("test_repos")
}

#[derive(Deserialize, Serialize)]
#[serde(remote = "ModuleKind", rename_all = "camelCase")]
pub enum ModuleKindDef {
    Script,
    Module,
}

#[derive(Deserialize, Serialize)]
pub struct SourceRecord {
    pub id: u64,
    pub path: String,
    #[serde(with = "ModuleKindDef")]
    pub module_kind: ModuleKind,
}
