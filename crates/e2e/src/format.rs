use std::sync::Arc;
use swc::{Compiler, PrintArgs};
use swc_common::{FileName, SourceMap, Spanned};
use swc_ecma_ast::{BigInt, EsVersion, Number, Program, Str};
use swc_ecma_parser::{with_file_parser, Syntax};
use swc_ecma_transforms::fixer::{fixer, paren_remover};

use swc_ecma_ast::{ClassMember, ModuleItem, Stmt};
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

    // Make printer generate consistent quote type instead of relying on the input
    fn visit_mut_number(&mut self, lit: &mut Number) {
        lit.raw = None;
    }
    fn visit_mut_str(&mut self, lit: &mut Str) {
        lit.raw = None;
    }
    
    fn visit_mut_big_int(&mut self, lit: &mut BigInt) {
        lit.raw = None;
    }

}

pub fn remove_empty_statements(node: &mut Program) {
    node.visit_mut_with(&mut EmptyStatementRemover);
}

pub fn format_js(source: &str) -> anyhow::Result<String> {
    let cm = Arc::new(SourceMap::default());
    let source = source.replace("@null", "@(null)");
    let fm = cm.new_source_file(FileName::Custom("a.js".into()).into(), source);
    let mut errors = vec![];

    let mut program = with_file_parser(
        &fm,
        Syntax::Es(swc_ecma_parser::EsSyntax {
            decorators: true,
            import_attributes: true,
            allow_return_outside_function: true,
            decorators_before_export: true,
            auto_accessors: true,
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

    let print_args = PrintArgs::default();
    
    let ret = compiler.print(
        &program, // ast to print
        print_args,
    )?;

    // swc preserves trailing comma in `export { a, } `
    // TODO: fix this in a proper way
    let code = ret.code.replace(",  }", " }");
    Ok(code)
}

