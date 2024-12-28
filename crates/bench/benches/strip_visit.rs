use oxc_allocator::{Allocator, Vec};
use oxc_span::{Span, GetSpan};
use oxc_ast::{ast, Visit, visit::walk};
use oxidase::Patch;

pub struct StripVisit<'alloc> {
    patches: Vec<'alloc, Patch<'static>>,
}

impl<'alloc> StripVisit<'alloc> {
    pub fn new(allocator: &'alloc Allocator) -> Self {
        Self {
            patches: Vec::new_in(allocator),
        }
    }
    pub fn into_patches(self) -> Vec<'alloc, Patch<'static>> {
        self.patches
    }

    #[inline]
    fn push_strip(&mut self, span: Span) {
        self.patches.push(Patch { span: (span.start..span.end).into(), replacement: "" });
    }
}


impl<'alloc, 'ast> Visit<'ast> for StripVisit<'alloc> {
    #[inline]
    fn visit_ts_type_annotation(&mut self, it: &ast::TSTypeAnnotation<'ast>) {
        self.push_strip(it.span);
    }
    #[inline]
    fn visit_ts_interface_declaration(&mut self, it: &ast::TSInterfaceDeclaration<'ast>) {
        self.push_strip(it.span);
    }

    #[inline]
    fn visit_ts_as_expression(&mut self, it: &ast::TSAsExpression<'ast>) {
        walk::walk_expression(self, &it.expression);
        self.push_strip(Span::new(it.expression.span().end, it.type_annotation.span().end));
    }

    #[inline]
    fn visit_ts_satisfies_expression(&mut self, it: &ast::TSSatisfiesExpression<'ast>) {
        walk::walk_expression(self, &it.expression);
        self.push_strip(Span::new(it.expression.span().end, it.type_annotation.span().end));
    }
}
