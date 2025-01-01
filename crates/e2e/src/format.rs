use std::sync::Arc;
use swc::{Compiler, PrintArgs};
use swc_common::{FileName, SourceMap, Spanned};
use swc_ecma_ast::{EsVersion, Program};
use swc_ecma_parser::{with_file_parser, Syntax};
use swc_ecma_transforms::fixer::{fixer, paren_remover};

use std::ops::Deref;
use swc_ecma_ast::{BindingIdent, ClassMember, Expr, Lit, ModuleItem, SimpleAssignTarget, Stmt};
use swc_ecma_visit::{VisitMut, VisitMutWith};

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
}

pub fn remove_empty_statements(node: &mut Program) {
    node.visit_mut_with(&mut EmptyStatementRemover);
}

pub fn format_js(source: impl Into<String>) -> anyhow::Result<String> {
    let cm = Arc::new(SourceMap::default());
    let fm = cm.new_source_file(FileName::Custom("a.js".into()).into(), source.into());
    let mut errors = vec![];

    let mut program = with_file_parser(
        &fm,
        Syntax::Es(swc_ecma_parser::EsSyntax {
            decorators: true,
            import_attributes: true,
            allow_return_outside_function: true,
            ..Default::default()
        }),
        EsVersion::latest(),
        None,
        &mut errors,
        |parser| Ok(Program::Module(parser.parse_module()?)),
    )
    .map_err(|err| anyhow::anyhow!("{:?}: {}", err.span(), err.kind().msg()))?;

    remove_empty_statements(&mut program);
    program = program.apply(paren_remover(None));
    program = program.apply(fixer(None));


    let compiler = Compiler::new(cm);

    let ret = compiler.print(
        &program, // ast to print
        PrintArgs::default(),
    )?;
    Ok(ret.code)
}

#[cfg(test)]
mod tests {
    use oxidase::{Allocator, SourceType};

    use super::*;

    #[test]
    fn test_format_ts() {
        let allocator = Allocator::default();
        let mut source = std::fs::read_to_string("/Users/patr0nus/code/oxidase/crates/e2e/fixtures/ecosystem/TypeScript/tests/cases/compiler/argumentsObjectIterator01_ES6.ts").unwrap();
        oxidase::transpile(&allocator, SourceType::ts(), &mut source);
        println!("--------------------\n{}", source);
        println!("--------------------\n{}", format_js(source.as_str()).unwrap());
    }
}
