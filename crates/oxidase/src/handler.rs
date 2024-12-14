use std::ops::Range;

use crate::patch::Patch;
use bumpalo::format;
use oxc_allocator::{Allocator, String, Vec};
use oxc_ast::handle::Handler as AstHandler;
use oxc_ast::{ast::*, AstScopeNode, ScopeType};
use oxc_parser::Handler as ParserHandler;
use oxc_span::ast_alloc::VoidAllocator;
use oxc_span::GetSpan;

trait SpanExt {
    fn range(self) -> Range<usize>;
}
impl SpanExt for Span {
    fn range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}


pub struct StripHandler<'source, 'alloc> {
    source: &'source str,
    allocator: &'alloc Allocator,

    patches: Vec<'alloc, Patch<'alloc>>,
    scope_stack: Vec<'alloc, Scope<'alloc>>,
}

#[derive(Clone, Copy, Debug)]
pub struct StripHandlerCheckpoint {
    patch_len: usize,
    scope_stack_len: usize,
}

#[derive(Debug)]

struct LastStatement {
    span: Span,
    is_first: bool,

    /// None if the statement's suffix/whole is not patched
    patch: Option<StatementPatch>,
}

#[derive(Debug)]
struct StatementPatch {
    index: usize,
    kind: StatementStripKind,
}

#[derive(Debug, Clone, Copy)]
enum StatementStripKind {
    Whole,
    Tail,
}

#[derive(Debug)]
struct Scope<'alloc> {
    last_statement: Option<LastStatement>,
    kind: ScopeKind<'alloc>,
}

#[derive(Debug)]
enum SuperCallFindingState {
    NotFound,
    ExpressionFound(Span),
    StatementFound { patch_index_after_stmt: usize },
}

#[derive(Debug)]
enum ScopeKind<'alloc> {
    Other,
    Class {
        current_element_first_modifier_patch_index: Option<usize>,
        parameter_prop_id_spans: Vec<'alloc, Span>,
    },
    ConstructorWithParamProps {
        parameter_prop_id_spans: Vec<'alloc, Span>,
        super_call_stmt_end: Option<u32>,
        last_super_call_expr_span: Option<Span>,
    },
}

impl<'source, 'alloc> StripHandler<'source, 'alloc> {
    pub fn new(allocator: &'alloc Allocator, source: &'source str) -> Self {
        Self {
            source,
            patches: Vec::new_in(allocator),
            allocator,
            scope_stack: Vec::with_capacity_in(32, allocator),
        }
    }
    pub fn into_patches(self) -> Vec<'alloc, Patch<'alloc>> {
        self.patches
    }
    fn push_strip(&mut self, span: Span) {
        self.push_patch(span, "");
    }

    fn push_patch(&mut self, span: Span, replacement: &'alloc str) {
        debug_assert!(span.end >= self.patches.last().map(|patch| patch.span.end).unwrap_or(0));

        while matches!(self.patches.last(), Some(patch) if patch.span.start >= span.start) {
            self.patches.pop();
        }
        self.patches.push(Patch { span, replacement });
    }
    fn insert_patch(&mut self, span: Span, replacement: &'alloc str) {
        // let mut insert_pos = self.patches.len();
        // while insert_pos > 0 && self.patches[insert_pos - 1].span.end > span.start {
        //     insert_pos -= 1
        // }
        // #[cfg(debug_assertions)]
        // if let Some(patch_after) = self.patches.get(insert_pos) {
        //     assert!(span.end <= patch_after.span.start);
        // }
        // self.patches.insert(insert_pos, Patch { span, replacement });
        self.patches.push(Patch { span, replacement });
    }
    // fn insert_strip(&mut self, span: Span) {
    //     self.insert_patch(span, "");
    // }

    fn source_bytes(&self) -> &[u8] {
        self.source.as_bytes()
    }

    fn cur_scope(&self) -> &Scope<'alloc> {
        self.scope_stack.last().unwrap()
    }
    fn cur_scope_mut(&mut self) -> &mut Scope<'alloc> {
        self.scope_stack.last_mut().unwrap()
    }

    fn non_block_body_asi(&mut self, span: Span) {
        let Some(last_patch) = self.patches.last_mut() else {
            return;
        };
        if last_patch.span == span && last_patch.replacement.is_empty() {
            last_patch.replacement = ";"
        }
    }

    fn class_element_modifiers_asi(&mut self) {
        let ScopeKind::Class {
            current_element_first_modifier_patch_index,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
        else {
            if cfg!(test) {
                unreachable!();
            }
            return;
        };
        let Some(current_element_first_modifier_patch_index) = *current_element_first_modifier_patch_index
        else {
            return;
        };
        let modifiers_patch =
            &mut self.patches.as_mut_slice()[current_element_first_modifier_patch_index];
        if !modifiers_patch.replacement.starts_with(";") {
            modifiers_patch.replacement =
                format!(in self.allocator, ";{}", modifiers_patch.replacement).into_bump_str();
        }
    }

    fn statement_asi(&mut self, span: Span) {
        let mut is_first = true;
        if let Some(last_statement) = &self.cur_scope().last_statement {
            is_first = false;

            if let (Some(StatementPatch { index, kind }), b'(' | b'[' | b'`' | b'+' | b'-' | b'/') = (
                &last_statement.patch,
                self.source.as_bytes()[span.start as usize],
            ) {
                let need_asi = matches!(
                    (kind, last_statement.is_first),
                    (StatementStripKind::Whole, false) | (StatementStripKind::Tail, _)
                );
                if need_asi {
                    let index = *index;
                    // TODO: Handle StatementStripKind::Tail (example: let a = 1 as string) with fewer first charactor matches.
                    let patch = &mut self.patches.as_mut_slice()[index];
                    if patch.replacement.is_empty() {
                        patch.replacement = ";"
                    } else {
                        patch.replacement =
                            format!(in self.allocator, "{};", patch.replacement).into_bump_str();
                    }
                }
            }
        }
        let mut current_stmt_patch: Option<StatementPatch> = None;
        if let Some(last_patch) = self.patches.last() {
            let index = self.patches.len() - 1;
            if last_patch.span.end == span.end {
                if last_patch.span.start == span.start {
                    current_stmt_patch = Some(StatementPatch {
                        index,
                        kind: StatementStripKind::Whole,
                    })
                } else {
                    current_stmt_patch = Some(StatementPatch {
                        index,
                        kind: StatementStripKind::Tail,
                    })
                }
            }
        }
        self.cur_scope_mut().last_statement = Some(LastStatement {
            span,
            is_first,
            patch: current_stmt_patch,
        });
    }
}

impl<'source, 'alloc, 'ast> ParserHandler<'ast, VoidAllocator> for StripHandler<'source, 'alloc> {
    type Checkpoint = StripHandlerCheckpoint;
    fn checkpoint(&self) -> Self::Checkpoint {
        StripHandlerCheckpoint {
            patch_len: self.patches.len(),
            scope_stack_len: self.scope_stack.len(),
        }
    }
    fn rewind(&mut self, checkpoint: Self::Checkpoint) {
        self.patches.truncate(checkpoint.patch_len);
        self.scope_stack.truncate(checkpoint.scope_stack_len);
    }
}

impl<'source, 'alloc, 'ast> AstHandler<'ast, VoidAllocator> for StripHandler<'source, 'alloc> {
    fn enter_scope<T: AstScopeNode>(&mut self) {
        let kind = match T::SCOPE_TYPE {
            ScopeType::Class => ScopeKind::Class {
                current_element_first_modifier_patch_index: None,
                parameter_prop_id_spans: Vec::new_in(&self.allocator),
            },
            _ => ScopeKind::Other,
        };
        self.scope_stack.push(Scope {
            last_statement: None,
            kind,
        });
    }
    fn leave_scope(&mut self) {
        let scope = self.scope_stack.pop().unwrap();
        match scope.kind {
            ScopeKind::ConstructorWithParamProps {
                parameter_prop_id_spans: parameter_prop_id_spans_under_constructor,
                ..
            } => {
                let Some(Scope {
                    kind:
                        ScopeKind::Class {
                            parameter_prop_id_spans: parameter_prop_id_spans_under_class,
                            ..
                        },
                    ..
                }) = self.scope_stack.last_mut()
                else {
                    return;
                };
                *parameter_prop_id_spans_under_class = parameter_prop_id_spans_under_constructor;
            }
            _ => {}
        }
    }

    fn handle_ts_export_assignment(
        &mut self,
        assignment: &TSExportAssignment<'ast, VoidAllocator>,
    ) {
        self.insert_patch(
            Span::new(assignment.span.start, assignment.expression.span().start),
            "module.exports = ",
        );
    }

    fn handle_export_specifier(&mut self, specifier: &ExportSpecifier<'ast>) {
        if specifier.export_kind.is_type() {
            self.push_strip(specifier.span);
        }
    }
    fn handle_import_specifier(&mut self, specifier: &ImportSpecifier<'ast>) {
        if specifier.import_kind.is_type() {
            self.push_strip(specifier.span);
        }
    }
    fn handle_export_named_declaration(
        &mut self,
        decl: &ExportNamedDeclaration<'ast, VoidAllocator>,
    ) {
        if decl.export_kind.is_type() {
            self.push_strip(decl.span);
            return;
        }
        let Some(exported_decl) = &decl.declaration else {
            return;
        };
        let Some(last_patch) = self.patches.last() else {
            return;
        };
        if (last_patch.span == exported_decl.span()) {
            self.push_strip(decl.span());
        }
    }
    fn handle_export_default_declaration(
        &mut self,
        decl: &ExportDefaultDeclaration<'ast, VoidAllocator>,
    ) {
        let Some(last_patch) = self.patches.last() else {
            return;
        };
        if (last_patch.span == decl.declaration.span()) {
            self.push_strip(decl.span());
        }
    }
    fn handle_export_all_declaration(&mut self, decl: &ExportAllDeclaration<'ast, VoidAllocator>) {
        if decl.export_kind.is_type() {
            self.push_strip(decl.span);
        }
    }
    fn handle_ts_class_implements(&mut self, implements: &TSClassImplements<'ast, VoidAllocator>) {
        self.push_strip(implements.span);
    }

    fn handle_variable_declaration(&mut self, decl: &VariableDeclaration<'ast, VoidAllocator>) {
        if decl.declare {
            self.push_strip(decl.span);
        }
    }
    fn handle_ts_interface_declaration(
        &mut self,
        interface_decl: &TSInterfaceDeclaration<'ast, VoidAllocator>,
    ) {
        self.push_strip(interface_decl.span);
    }
    fn handle_ts_enum_declaration(&mut self, enum_decl: &TSEnumDeclaration<'ast, VoidAllocator>) {
        if enum_decl.declare {
            self.push_strip(enum_decl.span);
        }
    }
    fn handle_ts_import_equals_declaration(
        &mut self,
        decl: &TSImportEqualsDeclaration<'ast, VoidAllocator>,
    ) {
        if decl.import_kind.is_type() {
            self.push_strip(decl.span);
            return;
        }
        let const_span = Span::new(decl.span.start, decl.id.span.start);
        self.insert_patch(const_span, "const ");
    }
    fn handle_import_declaration(&mut self, decl: &ImportDeclaration<'ast, VoidAllocator>) {
        if decl.import_kind.is_type() {
            self.push_strip(decl.span);
        }
    }
    fn handle_ts_module_declaration(&mut self, decl: &TSModuleDeclaration<'ast, VoidAllocator>) {
        if decl.declare {
            self.push_strip(decl.span);
        }
    }
    fn handle_ts_type_alias_declaration(
        &mut self,
        decl: &TSTypeAliasDeclaration<'ast, VoidAllocator>,
    ) {
        self.push_strip(decl.span);
    }

    fn handle_function_body(&mut self, body: &FunctionBody<'ast, VoidAllocator>) {
        let ScopeKind::ConstructorWithParamProps {
            parameter_prop_id_spans,
            super_call_stmt_end,
            ..
        } = &self.scope_stack.last().unwrap().kind
        else {
            return;
        };

        let mut prop_init_code: String<'_> = String::with_capacity_in(
            parameter_prop_id_spans
                .iter()
                .map(|id_span| {
                    " this.".len()
                        + id_span.size() as usize
                        + " = ".len()
                        + id_span.size() as usize
                        + ";".len()
                })
                .sum::<usize>()
                + 1,
            &self.allocator,
        );

        let insert_span = if let Some(super_call_stmt_end) = *super_call_stmt_end {
            prop_init_code.push(',');
            if self.source.as_bytes()[super_call_stmt_end as usize - 1] == b';' {
                Span::new(super_call_stmt_end - 1, super_call_stmt_end)
            } else {
                Span::new(super_call_stmt_end, super_call_stmt_end)
            }
        } else {
            let body_start = body.span().start;
            debug_assert_eq!(self.source_bytes()[body_start as usize], b'{');
            Span::new(body_start + 1, body_start + 1)
        };

        for id_span in parameter_prop_id_spans {
            let ident = &self.source[*id_span];
            prop_init_code.push_str(" this.");
            prop_init_code.push_str(ident);
            prop_init_code.push_str(" = ");
            prop_init_code.push_str(ident);
            prop_init_code.push_str(";");
        }
        let prop_init_code = prop_init_code.into_bump_str();

        self.insert_patch(insert_span, prop_init_code);
    }

    fn handle_function(&mut self, func: &Function<'ast, VoidAllocator>) {
        if func.declare || func.body.is_none() {
            self.push_strip(func.span);
        }
    }

    fn handle_class_element(&mut self, element: &ClassElement<'ast, VoidAllocator>) {
        let span = element.span();
        self.statement_asi(span);
        let ScopeKind::Class {
            current_element_first_modifier_patch_index,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
        else {
            if cfg!(debug_assertions) {
                panic!(
                    "Class element encountered in non-class scope: {:?}",
                    element.span()
                );
            }
            return;
        };
        *current_element_first_modifier_patch_index = None;
    }

    fn handle_statement(&mut self, stmt: &Statement<'ast, VoidAllocator>) {
        let stmt_span = stmt.span();
        self.statement_asi(stmt_span);
    }

    fn handle_expression_statement(
        &mut self,
        expr_stmt: &ExpressionStatement<'ast, VoidAllocator>,
    ) {
        if let ScopeKind::ConstructorWithParamProps {
            super_call_stmt_end,
            last_super_call_expr_span,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
         {
            // constructor (...) { ... }
            if let (Expression::CallExpression(call_expr), Some(last_super_call_expr_span)) = (&expr_stmt.expression, last_super_call_expr_span) {
                if call_expr.span() == *last_super_call_expr_span {
                    *super_call_stmt_end = Some(expr_stmt.span.end);
                }
            };
        };
    }

    fn handle_call_expression(&mut self, call_expr: &CallExpression<'ast, VoidAllocator>) {
       if matches!(call_expr.callee, Expression::Super(_)) {
        if let ScopeKind::ConstructorWithParamProps {
            last_super_call_expr_span,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind {
            *last_super_call_expr_span = Some(call_expr.span)
        }
       }
    }

    fn handle_ts_type_annotation(&mut self, it: &TSTypeAnnotation<'ast, VoidAllocator>) {
        self.push_strip(it.span);
    }
    fn handle_ts_type_parameter_declaration(
        &mut self,
        it: &TSTypeParameterDeclaration<'ast, VoidAllocator>,
    ) {
        self.push_strip(it.span);
    }
    fn handle_ts_type_parameter_instantiation(
        &mut self,
        it: &TSTypeParameterInstantiation<'ast, VoidAllocator>,
    ) {
        self.push_strip(it.span);
    }
    fn handle_ts_as_expression(&mut self, it: &TSAsExpression<'ast, VoidAllocator>) {
        self.push_strip((it.expression.span().end..it.span.end).into());
    }
    fn handle_ts_satisfies_expression(&mut self, it: &TSSatisfiesExpression<'ast, VoidAllocator>) {
        self.push_strip((it.expression.span().end..it.span.end).into());
    }
    fn handle_class_modifiers(&mut self, modifiers: &ClassModifiers) {
        if modifiers.r#abstract {
            self.push_strip(modifiers.span);
        }
    }

    fn handle_class_body(&mut self, class_body: &ClassBody<'ast, VoidAllocator>) {
        let Some(Scope {
            kind:
                ScopeKind::Class {
                    parameter_prop_id_spans,
                    ..
                },
            ..
        }) = self.scope_stack.last()
        else {
            panic!("Unexpected scope kind while handling class body");
        };
        let class_body_start = class_body.span.start;
        debug_assert_eq!(self.source_bytes()[class_body_start as usize], b'{');
        let mut prop_decls: String<'_> = String::with_capacity_in(parameter_prop_id_spans.iter().map(|span| span.size() as usize + 2).sum(), &self.allocator);
        for prop_id_span in parameter_prop_id_spans {
            prop_decls.push(' ');
            prop_decls.push_str(&self.source[*prop_id_span]);
            prop_decls.push(';');
        }
        self.insert_patch(Span::new(class_body_start + 1, class_body_start + 1), prop_decls.into_bump_str());
    }

    fn handle_class(&mut self, it: &Class<'ast, VoidAllocator>) {
        if it.modifiers.is_some_and(|modifiers| modifiers.declare) {
            self.push_strip(it.span);
            return;
        }
    }
    fn handle_ts_this_parameter(&mut self, it: &TSThisParameter<'ast, VoidAllocator>) {
        self.push_strip(it.span);
    }
    fn handle_ts_type_assertion_annotation(
        &mut self,
        annotation: &TSTypeAssertionAnnotation<'ast, VoidAllocator>,
    ) {
        self.push_strip(annotation.span);
    }
    fn handle_class_element_modifiers(&mut self, modifiers: &ClassElementModifiers) {
        if !(modifiers.r#abstract
            || modifiers.declare
            || modifiers.r#override
            || modifiers.readonly
            || modifiers.accessibility.is_some())
        {
            return;
        }

        let ScopeKind::Class {
            current_element_first_modifier_patch_index,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
        else {
            if cfg!(debug_assertions) {
                panic!(
                    "Class element modifiers encountered in non-class scope: {:?}",
                    modifiers.span
                );
            }
            return;
        };

        let modifiers_source = &self.source.as_bytes()[modifiers.span.range()];
        let mut start = 0usize;
        'scan_source: while start < modifiers_source.len() {
            const TS_MODIFIERS: &[&[u8]] = &[b"abstract", b"declare", b"override", b"readonly", b"private", b"protected", b"public"];
            for ts_modifier in TS_MODIFIERS {
                if modifiers_source[start..].starts_with(ts_modifier) {
                    self.patches.push(Patch { span: Span::new(modifiers.span.start + start as u32, modifiers.span.start + (start + ts_modifier.len()) as u32), replacement: "" });
                    current_element_first_modifier_patch_index.get_or_insert(self.patches.len() - 1);
                    start += ts_modifier.len();
                    continue 'scan_source;
                }
            }
            start += 1;
        }

    }

    fn handle_ts_definite_mark(&mut self, mark: &TSDefiniteMark) {
        self.push_strip(mark.span);
    }
    fn handle_ts_optional_mark(&mut self, mark: &TSOptionalMark) {
        self.push_strip(mark.span);
    }

    fn handle_method_definition(&mut self, element: &MethodDefinition<'ast, VoidAllocator>) {
        if matches!(self.patches.last(), Some(last_patch) if last_patch.span == element.value.span())
        {
            self.push_strip(element.span);
            return;
        }
        if element.kind.is_method() && (element.value.generator || element.key.is_expression()) {
            self.class_element_modifiers_asi();
        }
    }
    fn handle_property_definition(&mut self, element: &PropertyDefinition<'ast, VoidAllocator>) {
        if element
            .modifiers
            .is_some_and(|modifiers| modifiers.declare || modifiers.r#abstract)
        {
            self.push_strip(element.span);
            return;
        }
        if element.key.is_expression() {
            self.class_element_modifiers_asi();
        }
    }
    fn handle_accessor_property(&mut self, element: &AccessorProperty<'ast, VoidAllocator>) {
        if element
            .modifiers
            .is_some_and(|modifiers| modifiers.declare || modifiers.r#abstract)
        {
            self.push_strip(element.span);
        }
    }
    fn handle_ts_index_signature(&mut self, element: &TSIndexSignature<'ast, VoidAllocator>) {
        self.push_strip(element.span);
    }

    fn handle_arrow_function_expression(
        &mut self,
        arrow_func: &ArrowFunctionExpression<'ast, VoidAllocator>,
    ) {
        // () => <T>{ a: 1 } to
        // () => ({ a: 1 })
        if !arrow_func.expression {
            return;
        }
        let body_span = arrow_func.body.span();
        let source_bytes = self.source_bytes();

        // () => <....}
        if !(source_bytes[body_span.start as usize] == b'<'
            && source_bytes[(body_span.end - 1) as usize] == b'}')
        {
            return;
        }

        let Ok(type_assertion_patch_index) = self
            .patches
            .binary_search_by_key(&body_span.start, |patch| patch.span.start)
        else {
            #[cfg(test)]
            panic!("Failed to find the patch of type assertion (<...>) right after arrow (=>). Expected patch start: {}", body_span.start);
            return;
        };

        let type_assertion_patch = &mut self.patches.as_mut_slice()[type_assertion_patch_index];

        #[cfg(test)]
        if !type_assertion_patch.replacement.is_empty() {
            panic!("The patch replacement of type assertion (<...>) right after arrow (=>) is not empty: {}", type_assertion_patch.replacement);
        }
        type_assertion_patch.replacement = "(";
        self.push_patch(Span::new(body_span.end, body_span.end), ")");
    }

    fn handle_if_statement(&mut self, if_stmt: &IfStatement<'ast, VoidAllocator>) {
        if let (Some(alternate), Some(last_patch)) = (&if_stmt.alternate, self.patches.last_mut()) {
            if last_patch.span == alternate.span() && last_patch.replacement.is_empty() {
                last_patch.replacement = ";"
            }
        }
        let consequent_span = if_stmt.consequent.span();
        let possible_strip_patch_of_consequent = if if_stmt.alternate.is_none() {
            self.patches.last_mut()
        } else {
            if let Ok(index) = self
                .patches
                .binary_search_by_key(&consequent_span.start, |patch| patch.span.start)
            {
                Some(&mut self.patches.as_mut_slice()[index])
            } else {
                None
            }
        };
        let Some(possible_strip_patch_of_consequent) = possible_strip_patch_of_consequent else {
            return;
        };
        if possible_strip_patch_of_consequent.span == if_stmt.consequent.span()
            && possible_strip_patch_of_consequent.replacement.is_empty()
        {
            possible_strip_patch_of_consequent.replacement = ";"
        }
    }
    fn handle_while_statement(&mut self, stmt: &WhileStatement<'ast, VoidAllocator>) {
        self.non_block_body_asi(stmt.body.span());
    }
    fn handle_for_statement(&mut self, stmt: &ForStatement<'ast, VoidAllocator>) {
        self.non_block_body_asi(stmt.body.span());
    }
    fn handle_for_in_statement(&mut self, stmt: &ForInStatement<'ast, VoidAllocator>) {
        self.non_block_body_asi(stmt.body.span());
    }
    fn handle_for_of_statement(&mut self, stmt: &ForOfStatement<'ast, VoidAllocator>) {
        self.non_block_body_asi(stmt.body.span());
    }

    fn handle_formal_parameter_modifiers(&mut self, modifiers: &FormalParameterModifiers) {
        self.push_strip(modifiers.span);
    }

    fn handle_formal_parameter(&mut self, param: &FormalParameter<'ast, VoidAllocator>) {
        if param.modifiers.is_none() {
            return;
        }
        let BindingPatternKind::BindingIdentifier(param_id) = &param.pattern.kind else {
            return;
        };
        let param_id_span = param_id.span();

        let current_scope_kind = &mut self.scope_stack.last_mut().unwrap().kind;
        match current_scope_kind {
            ScopeKind::Other => {
                *current_scope_kind = ScopeKind::ConstructorWithParamProps {
                    super_call_stmt_end: None,
                    parameter_prop_id_spans: {
                        let mut prop_id_spans = Vec::with_capacity_in(1, &self.allocator);
                        prop_id_spans.push(param_id_span);
                        prop_id_spans
                    },
                    last_super_call_expr_span: None,
                }
            }
            ScopeKind::ConstructorWithParamProps {
                parameter_prop_id_spans,
                ..
            } => {
                parameter_prop_id_spans.push(param_id_span);
            }
            other => panic!(
                "Formal parameter encountered in unexpected scope: {:?}",
                other
            ),
        };
    }
}
