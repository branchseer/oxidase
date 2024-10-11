use crate::patch::Patch;
use oxc_allocator::{Allocator, Vec};
use oxc_ast::ast::*;
use oxc_ast::Visit;

pub struct Visitor<'alloc> {
    patches: Vec<'alloc, Patch<'alloc>>,
    allocator: &'alloc Allocator,
}
impl<'alloc> Visitor<'alloc> {
    pub fn new(allocator: &'alloc Allocator) -> Self {
        Self {
            patches: Vec::new_in(allocator),
            allocator,
        }
    }
    pub fn into_patches(self) -> Vec<'alloc, Patch<'alloc>> {
        self.patches
    }
    fn push_strip(&mut self, span: Span) {
        self.patches.push(Patch {
            span,
            replacement: "",
        });
    }
}

impl<'alloc, 'ast> Visit<'ast> for Visitor<'alloc> {
    fn visit_ts_interface_declaration(&mut self, it: &TSInterfaceDeclaration<'ast>) {
        self.push_strip(it.span);
    }
    fn visit_ts_type_annotation(&mut self, it: &TSTypeAnnotation<'ast>) {
        self.push_strip(it.span);
    }
}
