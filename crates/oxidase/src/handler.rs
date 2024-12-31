use std::ops::Range;

use crate::patch::Patch;
use crate::patch_builder::PatchBuilder;
use bumpalo::format;
use oxc_allocator::{Allocator, String, Vec};
use oxc_ast::handle::Handler as AstHandler;
use oxc_ast::{ast::*, AstScopeNode, ScopeType};
use oxc_parser::Handler as ParserHandler;
use oxc_span::ast_alloc::AstAllocator;
use oxc_span::GetSpan;

trait SpanExt {
    #[inline]
    fn range(self) -> Range<usize>;
}
impl SpanExt for Span {
    #[inline]
    fn range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

pub struct StripHandler<'source, 'alloc> {
    source: &'source str,
    allocator: &'alloc Allocator,

    patches: PatchBuilder<'source, 'alloc>,
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
        parameter_prop_id_spans_in_first_constructor: Vec<'alloc, Span>,
        parameter_prop_id_spans: Vec<'alloc, Span>,
        parameter_prop_init_insert_start: Option<u32>,
    },
    FunctionWithParamProps {
        parameter_prop_id_spans: Vec<'alloc, Span>,
        super_call_stmt_end: Option<u32>,
        last_super_call_expr_span: Option<Span>,
        prologue_scan_state: PrologueScanState,
    },
}

#[derive(Debug)]
enum PrologueScanState {
    Init,
    InPrologues { last_prologue_stmt_end: u32 },
    End { last_prologue_stmt_end: Option<u32> },
}

impl<'source, 'alloc> StripHandler<'source, 'alloc> {
    pub fn new(allocator: &'alloc Allocator, source: &'source str) -> Self {
        Self {
            source,
            patches: PatchBuilder::new(source.as_bytes(), allocator),
            allocator,
            scope_stack: Vec::with_capacity_in(32, allocator),
        }
    }
    pub fn into_patches(self) -> Vec<'alloc, Patch<'alloc>> {
        self.patches.into_patches()
    }
    #[inline]
    fn source_bytes(&self) -> &[u8] {
        self.source.as_bytes()
    }

    #[inline]
    fn cur_scope(&self) -> &Scope<'alloc> {
        self.scope_stack.last().unwrap()
    }
    #[inline]
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

    // If the prefix of a class element is stripped, insert a ";" there;
    // `readonly ['a'] = 1` -> `; ['a'] = 1`
    fn class_element_prefix_patch_asi(&mut self, element_start: u32) {
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
        let Some(current_element_first_modifier_patch_index) =
            *current_element_first_modifier_patch_index
        else {
            return;
        };
        let first_modifier_patch =
            &mut self.patches[current_element_first_modifier_patch_index];

        // first_modifier_patch isn't always the prefix of a class element：
        // @foo readonly ['a'] = 1;
        // static readonly ['a'] = 1;
        if first_modifier_patch.span.start == element_start
            && !first_modifier_patch.replacement.starts_with(";")
        {
            first_modifier_patch.replacement =
                format!(in self.allocator, ";{}", first_modifier_patch.replacement).into_bump_str();
        }
    }

    #[inline]
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
                    let patch = &mut self.patches[index];
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

impl<'source, 'alloc, 'ast, A: AstAllocator> ParserHandler<'ast, A>
    for StripHandler<'source, 'alloc>
{
    type Checkpoint = StripHandlerCheckpoint;
    #[inline]
    fn checkpoint(&self) -> Self::Checkpoint {
        StripHandlerCheckpoint {
            patch_len: self.patches.len(),
            scope_stack_len: self.scope_stack.len(),
        }
    }
    #[inline]
    fn rewind(&mut self, checkpoint: Self::Checkpoint) {
        self.patches.truncate(checkpoint.patch_len);
        self.scope_stack.truncate(checkpoint.scope_stack_len);
    }
}

impl<'source, 'alloc, 'ast, A: AstAllocator> AstHandler<'ast, A> for StripHandler<'source, 'alloc> {
    #[inline]
    fn enter_scope<T: AstScopeNode>(&mut self) {
        let kind = match T::SCOPE_TYPE {
            ScopeType::Class => ScopeKind::Class {
                current_element_first_modifier_patch_index: None,
                parameter_prop_id_spans_in_first_constructor: Vec::new_in(&self.allocator),
                parameter_prop_id_spans: Vec::new_in(&self.allocator),
                parameter_prop_init_insert_start: None,
            },
            // ScopeType::TSEnumDeclaration => {
            //     let patch_before_enum_id_index = self.patches.len();
            //     self.patches.push(Patch::default());
            //     let patch_after_enum_id_index = self.patches.len();
            //     self.patches.push(Patch::default());

            //     ScopeKind::Enum(EnumScope {
            //         patch_before_enum_id_index,
            //         patch_after_enum_id_index,
            //     })
            // }
            _ => ScopeKind::Other,
        };
        self.scope_stack.push(Scope {
            last_statement: None,
            kind,
        });
    }

    #[inline]
    fn leave_scope(&mut self) {
        let scope = self.scope_stack.pop().unwrap();
        match scope.kind {
            ScopeKind::FunctionWithParamProps {
                parameter_prop_id_spans: parameter_prop_id_spans_under_function,
                super_call_stmt_end,
                prologue_scan_state,
                ..
            } => {
                let Some(Scope {
                    kind:
                        ScopeKind::Class {
                            parameter_prop_id_spans_in_first_constructor,
                            parameter_prop_id_spans: parameter_prop_id_spans_under_class,
                            parameter_prop_init_insert_start,
                            ..
                        },
                    ..
                }) = self.scope_stack.last_mut()
                else {
                    return;
                };

                /*
                   Because this `leave_scope` is triggered by the function part of the method, we can know if the function is a constructor or not
                   (e.g. someMethod(public a) {} ),
                   The following scope field updates may be correct, they will be cleared if it's not constructor in handle_method_definition.
                */
                if parameter_prop_id_spans_in_first_constructor.is_empty() {
                    /* Remember param props in the first constructor for emiting field declarations:
                    class A {
                        a <--- remember a for this
                        constructor(private a) {}
                        constructor(private b) {}
                    }
                    */
                    parameter_prop_id_spans_in_first_constructor
                        .extend_from_slice(&parameter_prop_id_spans_under_function);
                }
                *parameter_prop_id_spans_under_class = parameter_prop_id_spans_under_function;
                *parameter_prop_init_insert_start =
                    super_call_stmt_end.or_else(|| match prologue_scan_state {
                        PrologueScanState::InPrologues {
                            last_prologue_stmt_end,
                        } => Some(last_prologue_stmt_end),
                        PrologueScanState::End {
                            last_prologue_stmt_end,
                        } => last_prologue_stmt_end,
                        PrologueScanState::Init => None,
                    })
            }
            _ => {}
        }
    }

    #[inline]
    fn handle_ts_export_assignment(&mut self, assignment: &TSExportAssignment<'ast, A>) {
        self.patches.binary_search_insert((
            Span::new(assignment.span.start, assignment.expression.span().start),
            "module.exports = ",
        ));
    }

    #[inline]
    fn handle_export_specifier(&mut self, specifier: &ExportSpecifier<'ast>) {
        if specifier.export_kind.is_type() {
            self.patches.push(specifier.span);
        }
    }
    #[inline]
    fn handle_import_specifier(&mut self, specifier: &ImportSpecifier<'ast>) {
        if specifier.import_kind.is_type() {
            self.patches.push(specifier.span);
        }
    }
    #[inline]
    fn handle_ts_namespace_export_declaration(
        &mut self,
        decl: &TSNamespaceExportDeclaration<'ast>,
    ) {
        // export as namespace Foo;
        self.patches.push(decl.span);
    }
    #[inline]
    fn handle_export_named_declaration(&mut self, decl: &ExportNamedDeclaration<'ast, A>) {
        if decl.export_kind.is_type() {
            self.patches.push(decl.span);
            return;
        }
        let Some(exported_decl) = &decl.declaration else {
            return;
        };
        let Some(last_patch) = self.patches.last() else {
            return;
        };
        if last_patch.span == exported_decl.span() {
            self.patches.push_merging_tail(decl.span());
        }
    }
    #[inline]
    fn handle_export_default_declaration(&mut self, decl: &ExportDefaultDeclaration<'ast, A>) {
        let Some(last_patch) = self.patches.last() else {
            return;
        };
        if (last_patch.span == decl.declaration.span()) {
            self.patches.push_merging_tail(decl.span());
        }
    }
    #[inline]
    fn handle_export_all_declaration(&mut self, decl: &ExportAllDeclaration<'ast, A>) {
        if decl.export_kind.is_type() {
            self.patches.push_merging_tail(decl.span);
        }
    }
    #[inline]
    fn handle_ts_class_implements(&mut self, implements: &TSClassImplements<'ast, A>) {
        self.patches.push_merging_tail(implements.span);
    }

    #[inline]
    fn handle_variable_declaration(&mut self, decl: &VariableDeclaration<'ast, A>) {
        if decl.declare {
            self.patches.push_merging_tail(decl.span);
        }
    }

    #[inline]
    fn handle_ts_interface_declaration(
        &mut self,
        interface_decl: &TSInterfaceDeclaration<'ast, A>,
    ) {
        self.patches.push_merging_tail(interface_decl.span);
    }

    fn handle_ts_enum_head(&mut self, enum_head: &TSEnumHead<'ast>) {
        // `(const) enum A {` -> `var A; (function (A) {`
        let id = &self.source[enum_head.id.span.range()];

        // There could be line terminators between `const` and `enum`
        self.patches.push_checking_line_terminator(Patch {
            span: (enum_head.span.start..enum_head.id.span.start).into(),
            replacement: format!(in &self.allocator, "var {}; (function (", id).into_bump_str(),
        });

        self.patches.push(Patch {
            span: (enum_head.id.span.end..enum_head.id.span.end).into(),
            replacement: ")",
        });
    }

    #[inline]
    fn handle_ts_enum_declaration(&mut self, enum_decl: &TSEnumDeclaration<'ast, A>) {
        if enum_decl.head.declare {
            self.patches.push_merging_tail(enum_decl.span);
            return;
        }
        let id = &self.source[enum_decl.head.id.span.range()];

        self.patches.push(Patch { span: (enum_decl.span.end..enum_decl.span.end).into(), replacement: "" });
    }
    #[inline]
    fn handle_ts_import_equals_declaration(&mut self, decl: &TSImportEqualsDeclaration<'ast, A>) {
        if decl.import_kind.is_type() {
            self.patches.push_merging_tail(decl.span);
            return;
        }
        let const_span = Span::new(decl.span.start, decl.id.span.start);
        self.patches.binary_search_insert((
            const_span,
            match decl.module_reference {
                TSModuleReference::ExternalModuleReference(_) => "const ",
                _ => "var ",
            },
        ));
    }
    #[inline]
    fn handle_import_declaration(&mut self, decl: &ImportDeclaration<'ast, A>) {
        if decl.import_kind.is_type() {
            self.patches.push_merging_tail(decl.span);
        }
    }
    #[inline]
    fn handle_ts_module_declaration(&mut self, decl: &TSModuleDeclaration<'ast, A>) {
        if decl.declare {
            self.patches.push_merging_tail(decl.span);
        }
    }
    #[inline]
    fn handle_ts_type_alias_declaration(&mut self, decl: &TSTypeAliasDeclaration<'ast, A>) {
        self.patches.push_merging_tail(decl.span);
    }

    #[inline]
    fn handle_function_body(&mut self, body: &FunctionBody<'ast, A>) {}

    #[inline]
    fn handle_function(&mut self, func: &Function<'ast, A>) {
        if func.declare || func.body.is_none() {
            self.patches.push_merging_tail(func.span);
        }
    }

    #[inline]
    fn handle_class_element(&mut self, element: &ClassElement<'ast, A>) {
        let span = element.span();
        self.class_element_prefix_patch_asi(span.start);
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

    #[inline]
    fn handle_statement(&mut self, stmt: &Statement<'ast, A>) {
        let stmt_span = stmt.span();
        self.statement_asi(stmt_span);
        if let ScopeKind::FunctionWithParamProps {
            prologue_scan_state,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
        {
            match prologue_scan_state {
                PrologueScanState::Init => {
                    *prologue_scan_state = PrologueScanState::End {
                        last_prologue_stmt_end: None,
                    }
                }
                PrologueScanState::InPrologues {
                    last_prologue_stmt_end,
                } => {
                    if *last_prologue_stmt_end != stmt_span.end {
                        *prologue_scan_state = PrologueScanState::End {
                            last_prologue_stmt_end: Some(*last_prologue_stmt_end),
                        }
                    }
                }
                PrologueScanState::End { .. } => {}
            }
        }
    }

    #[inline]
    fn handle_expression_statement(&mut self, expr_stmt: &ExpressionStatement<'ast, A>) {
        if let ScopeKind::FunctionWithParamProps {
            super_call_stmt_end,
            last_super_call_expr_span,
            prologue_scan_state,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
        {
            // constructor (...) { ... }
            if let (Expression::CallExpression(call_expr), Some(last_super_call_expr_span)) =
                (&expr_stmt.expression, last_super_call_expr_span)
            {
                if call_expr.span() == *last_super_call_expr_span {
                    *super_call_stmt_end = Some(expr_stmt.span.end);
                }
            }
            if let Expression::StringLiteral(_) = expr_stmt.expression {
                if matches!(
                    prologue_scan_state,
                    PrologueScanState::Init | PrologueScanState::InPrologues { .. }
                ) {
                    *prologue_scan_state = PrologueScanState::InPrologues {
                        last_prologue_stmt_end: expr_stmt.span.end,
                    }
                }
            }
        };
    }

    #[inline]
    fn handle_call_expression(&mut self, call_expr: &CallExpression<'ast, A>) {
        if matches!(call_expr.callee, Expression::Super(_)) {
            if let ScopeKind::FunctionWithParamProps {
                last_super_call_expr_span,
                ..
            } = &mut self.scope_stack.last_mut().unwrap().kind
            {
                *last_super_call_expr_span = Some(call_expr.span)
            }
        }
    }

    #[inline]
    fn handle_ts_type_annotation(&mut self, it: &TSTypeAnnotation<'ast, A>) {
        self.patches.push_merging_tail(it.span);
    }
    #[inline]
    fn handle_ts_type_parameter_declaration(&mut self, it: &TSTypeParameterDeclaration<'ast, A>) {
        self.patches.push_merging_tail(it.span);
    }
    #[inline]
    fn handle_ts_type_parameter_instantiation(
        &mut self,
        it: &TSTypeParameterInstantiation<'ast, A>,
    ) {
        self.patches.push_merging_tail(it.span);
    }
    #[inline]
    fn handle_ts_as_expression(&mut self, it: &TSAsExpression<'ast, A>) {
        self.patches.push_merging_tail(it.expression.span().end..it.span.end);
    }
    #[inline]
    fn handle_ts_satisfies_expression(&mut self, it: &TSSatisfiesExpression<'ast, A>) {
        self.patches.push_merging_tail(it.expression.span().end..it.span.end);
    }
    #[inline]
    fn handle_class_modifiers(&mut self, modifiers: &ClassModifiers) {
        if modifiers.r#abstract {
            self.patches.push_merging_tail(modifiers.span);
        }
    }

    #[inline]
    fn handle_class_body(&mut self, class_body: &ClassBody<'ast, A>) {
        let Some(Scope {
            kind:
                ScopeKind::Class {
                    parameter_prop_id_spans_in_first_constructor,
                    ..
                },
            ..
        }) = self.scope_stack.last()
        else {
            panic!("Unexpected scope kind while handling class body");
        };
        let class_body_start = class_body.span.start;
        debug_assert_eq!(self.source_bytes()[class_body_start as usize], b'{');
        let mut prop_decls: String<'_> = String::with_capacity_in(
            parameter_prop_id_spans_in_first_constructor
                .iter()
                .map(|span| span.size() as usize + 2)
                .sum(),
            &self.allocator,
        );
        for prop_id_span in parameter_prop_id_spans_in_first_constructor {
            prop_decls.push(' ');
            prop_decls.push_str(&self.source[*prop_id_span]);
            prop_decls.push(';');
        }
        self.patches.binary_search_insert((
            Span::new(class_body_start + 1, class_body_start + 1),
            prop_decls.into_bump_str(),
        ));
    }

    #[inline]
    fn handle_class(&mut self, it: &Class<'ast, A>) {
        if it.modifiers.is_some_and(|modifiers| modifiers.declare) {
            self.patches.push_merging_tail(it.span);
            return;
        }
    }
    #[inline]
    fn handle_ts_this_parameter(&mut self, it: &TSThisParameter<'ast, A>) {
        self.patches.push_merging_tail(it.span);
    }
    #[inline]
    fn handle_ts_function_type(&mut self, ts_func_type: &TSFunctionType<'ast, A>) {
        // ignore param props in function types (`constructor(a: (public b) => void) {}`)
        if let ScopeKind::FunctionWithParamProps {
            parameter_prop_id_spans,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
        {
            let ts_func_type_start = ts_func_type.span.start;
            while matches!(parameter_prop_id_spans.last(), Some(span) if span.start >= ts_func_type_start)
            {
                parameter_prop_id_spans.pop();
            }
        }
    }
    #[inline]
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
            // ClassElementModifiers could also be encountered in types e.g. `{ readonly a: string }`
            return;
        };

        let modifiers_source = &self.source.as_bytes()[modifiers.span.range()];
        let mut start = 0usize;
        'scan_source: while start < modifiers_source.len() {
            const TS_MODIFIERS: &[&[u8]] = &[
                b"abstract",
                b"declare",
                b"override",
                b"readonly",
                b"private",
                b"protected",
                b"public",
            ];
            for ts_modifier in TS_MODIFIERS {
                if modifiers_source[start..].starts_with(ts_modifier) {
                    self.patches.push(Patch {
                        span: Span::new(
                            modifiers.span.start + start as u32,
                            modifiers.span.start + (start + ts_modifier.len()) as u32,
                        ),
                        replacement: "",
                    });
                    current_element_first_modifier_patch_index
                        .get_or_insert(self.patches.len() - 1);
                    start += ts_modifier.len();
                    continue 'scan_source;
                }
            }
            start += 1;
        }
    }

    #[inline]
    fn handle_ts_definite_mark(&mut self, mark: &TSDefiniteMark) {
        self.patches.push_merging_tail(mark.span);
    }
    #[inline]
    fn handle_ts_optional_mark(&mut self, mark: &TSOptionalMark) {
        self.patches.push_merging_tail(mark.span);
    }

    #[inline]
    fn handle_method_definition(&mut self, element: &MethodDefinition<'ast, A>) {
        if matches!(self.patches.last(), Some(last_patch) if last_patch.span == element.value.span() && last_patch.replacement.is_empty())
        {
            // if the function part is stripped (declare or empty body), strip the whole method
            self.patches.push_merging_tail(element.span);
        }

        if let ScopeKind::Class {
            parameter_prop_id_spans_in_first_constructor,
            parameter_prop_id_spans,
            parameter_prop_init_insert_start,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
        {
            if let (MethodDefinitionKind::Constructor, Some(body)) =
                (element.kind, &element.value.body)
            {
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

                let insert_span = if let Some(parameter_prop_init_insert_start) =
                    *parameter_prop_init_insert_start
                {
                    prop_init_code.push(';');
                    if self.source.as_bytes()[parameter_prop_init_insert_start as usize - 1] == b';'
                    {
                        Span::new(
                            parameter_prop_init_insert_start - 1,
                            parameter_prop_init_insert_start,
                        )
                    } else {
                        Span::new(
                            parameter_prop_init_insert_start,
                            parameter_prop_init_insert_start,
                        )
                    }
                } else {
                    let body_start = body.span().start;
                    debug_assert_eq!(self.source.as_bytes()[body_start as usize], b'{');
                    Span::new(body_start + 1, body_start + 1)
                };

                for id_span in parameter_prop_id_spans.iter() {
                    let ident = &self.source[*id_span];
                    prop_init_code.push_str(" this.");
                    prop_init_code.push_str(ident);
                    prop_init_code.push_str(" = ");
                    prop_init_code.push_str(ident);
                    prop_init_code.push_str(";");
                }
                let prop_init_code = prop_init_code.into_bump_str();

                self.patches.binary_search_insert((insert_span, prop_init_code));
            } else {
                // clear param prop state in class scope if the method isn't constructor or the constructor body is empty (someMethod(public a)),
                // to avoid emiting field declarations.
                if matches!(parameter_prop_id_spans_in_first_constructor.first(), Some(id_span) if id_span.start >= element.span.start)
                {
                    parameter_prop_id_spans_in_first_constructor.clear();
                }
                parameter_prop_id_spans.clear();
                *parameter_prop_init_insert_start = None;
            }
        };
    }
    #[inline]
    fn handle_property_definition(&mut self, element: &PropertyDefinition<'ast, A>) {
        if element
            .modifiers
            .is_some_and(|modifiers| modifiers.declare || modifiers.r#abstract)
        {
            self.patches.push_merging_tail(element.span);
            return;
        }
    }
    #[inline]
    fn handle_accessor_property(&mut self, element: &AccessorProperty<'ast, A>) {
        if element
            .modifiers
            .is_some_and(|modifiers| modifiers.declare || modifiers.r#abstract)
        {
            self.patches.push_merging_tail(element.span);
        }
    }
    #[inline]
    fn handle_ts_index_signature(&mut self, element: &TSIndexSignature<'ast, A>) {
        self.patches.push_merging_tail(element.span);
    }

    #[inline]
    fn handle_object_property(&mut self, prop: &ObjectProperty<'ast, A>) {
        if prop.method {
            if let (Some(patch), Expression::FunctionExpression(function_value)) =
                (self.patches.last(), &prop.value)
            {
                if patch.span == function_value.span() {
                    self.patches.push_merging_tail(prop.span);
                }
            }
        }
        // if prop.method && matches(&prop.value, Expression::
    }

    #[inline]
    fn handle_arrow_function_expression(&mut self, arrow_func: &ArrowFunctionExpression<'ast, A>) {
        // `()
        // => ...`
        // to
        // `(
        // ) => ...`
        if let Some(return_type) = &arrow_func.return_type {
            if let Ok(return_type_strip_patch_index) = self
                .patches
                .binary_search_by_key(&return_type.span().start, |patch| patch.span.start)
            {
                let strip_patch = &mut self.patches[return_type_strip_patch_index];
                debug_assert_eq!(strip_patch.span, return_type.span());

                // strips from the closing parenthesis of the params
                strip_patch.span.start = arrow_func.params.span().end - 1;
                debug_assert_eq!(self.source.as_bytes()[strip_patch.span.start as usize], b')', "expect arrow function with return type annotation to have closing parenthesis after params");

                // replace the return type annoations's last character (which should always be in the same line as the arrow token) with closing parenthesis of the params
                strip_patch.span.end -= 1;
                let closing_parenthesis_span =
                    Span::new(strip_patch.span.end, strip_patch.span.end + 1);
                self.patches.insert(
                    return_type_strip_patch_index + 1,
                    Patch {
                        span: closing_parenthesis_span,
                        replacement: ")",
                    },
                );
            } else {
                #[cfg(debug_assertions)]
                panic!("Failed to find the patch to strip the return type annotaion of an arrow function (annocation span: {:?})", return_type.span());
            }
        }
    }

    // wrap expression of TSTypeAssertion with () to handle:
    // `() => <Foo>{ foo: 1 }`, and
    // `return <{
    // }>{}`
    #[inline]
    fn handle_ts_type_assertion_annotation(
        &mut self,
        assertion_annotaion: &TSTypeAssertionAnnotation<'ast, A>,
    ) {
        self.patches.push_merging_tail((assertion_annotaion.span, "("));
    }
    #[inline]
    fn handle_ts_type_assertion(&mut self, type_assertion: &TSTypeAssertion<'ast, A>) {
        self.patches.push(Patch {
            span: (type_assertion.span.end..type_assertion.span.end).into(),
            replacement: ")",
        });
    }

    #[inline]
    fn handle_if_statement(&mut self, if_stmt: &IfStatement<'ast, A>) {
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
                Some(&mut self.patches[index])
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
    #[inline]
    fn handle_do_while_statement(&mut self, stmt: &DoWhileStatement<'ast, A>) {
        self.non_block_body_asi(stmt.body.span());
    }
    #[inline]
    fn handle_while_statement(&mut self, stmt: &WhileStatement<'ast, A>) {
        self.non_block_body_asi(stmt.body.span());
    }
    #[inline]
    fn handle_for_statement(&mut self, stmt: &ForStatement<'ast, A>) {
        self.non_block_body_asi(stmt.body.span());
    }
    #[inline]
    fn handle_for_in_statement(&mut self, stmt: &ForInStatement<'ast, A>) {
        self.non_block_body_asi(stmt.body.span());
    }
    #[inline]
    fn handle_for_of_statement(&mut self, stmt: &ForOfStatement<'ast, A>) {
        self.non_block_body_asi(stmt.body.span());
    }

    #[inline]
    fn handle_formal_parameter_modifiers(&mut self, modifiers: &FormalParameterModifiers) {
        self.patches.push_merging_tail(modifiers.span);
    }

    #[inline]
    fn handle_formal_parameter(&mut self, param: &FormalParameter<'ast, A>) {
        if param.modifiers.is_none() {
            return;
        }
        let param_id_span = match &param.pattern.kind {
            BindingPatternKind::BindingIdentifier(param_id) => {
                let mut param_id_span = param_id.span();

                if let Some(optional_mark) = &param.pattern.optional {
                    param_id_span.end = optional_mark.span.start;
                } else if let Some(type_annotation) = &param.pattern.type_annotation {
                    param_id_span.end = type_annotation.span().start;
                }
                param_id_span
            }
            BindingPatternKind::AssignmentPattern(assign_pat) => {
                // public ... = 1
                let start = assign_pat.span().start;

                if matches!(self.source_bytes()[start as usize], b'{' | b'[') {
                    // public {a} = {}
                    return;
                }
                let mut end = start;
                while end <= assign_pat.span().end {
                    if matches!(
                        self.source_bytes()[end as usize],
                        b'?' | b'=' | b',' | b')' | b'/' | b':'
                    ) {
                        break;
                    }
                    end += 1
                }
                Span::new(start, end)
            }
            _ => return,
        };

        let current_scope_kind = &mut self.scope_stack.last_mut().unwrap().kind;
        match current_scope_kind {
            ScopeKind::Other => {
                *current_scope_kind = ScopeKind::FunctionWithParamProps {
                    super_call_stmt_end: None,
                    parameter_prop_id_spans: {
                        let mut prop_id_spans = Vec::with_capacity_in(1, &self.allocator);
                        prop_id_spans.push(param_id_span);
                        prop_id_spans
                    },
                    last_super_call_expr_span: None,
                    prologue_scan_state: PrologueScanState::Init,
                }
            }
            ScopeKind::FunctionWithParamProps {
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
