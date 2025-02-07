use std::fmt::Write;
use std::ops::Range;

use crate::patch::Patch;
use crate::patch_builder::PatchBuilder;
use bumpalo::{format, Bump};
use hashbrown::hash_map::DefaultHashBuilder;
use hashbrown::{HashMap, HashSet};
use oxc_allocator::{Allocator, String, Vec};
use oxc_ast::handle::Handler as AstHandler;
use oxc_ast::{ast::*, AstScopeNode, ScopeType};
use oxc_data_structures::stack::NonEmptyStack;
use oxc_parser::Handler as ParserHandler;
use oxc_span::ast_alloc::AstAllocator;
use oxc_span::GetSpan;
use oxc_syntax::identifier::is_identifier_name;

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

    patches: PatchBuilder<'alloc>,
    scope_stack: NonEmptyStack<Scope<'alloc>>,
}

#[derive(Clone, Copy, Debug)]
pub struct StripHandlerCheckpoint {
    patch_len: u32,
    scope_stack_len: u32,
}

#[derive(Debug)]

struct LastStatement {
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
struct CurrentEnumDeclaration<'alloc> {
    enum_name: &'alloc str,
    index_of_patch_before_enum_name: usize,
    is_secondary: bool,
}

#[derive(Debug)]
struct CurrentNamespaceDeclaration<'alloc> {
    namespace_name: &'alloc str,
    index_of_patch_before_namespace_name: usize,
    is_ambient: bool,
}

#[derive(Debug)]
struct Scope<'alloc> {
    last_statement: Option<LastStatement>,
    kind: ScopeKind<'alloc>,
    member_identifiers_by_enum_names: HashMap<
        &'alloc str,
        HashSet<&'alloc str, DefaultHashBuilder, &'alloc Bump>,
        DefaultHashBuilder,
        &'alloc Bump,
    >,
    current_enum_decl: Option<CurrentEnumDeclaration<'alloc>>,
    current_namespace_decl: Option<CurrentNamespaceDeclaration<'alloc>>,
}

#[derive(Debug)]
struct ClassScope<'alloc> {
    current_element_first_modifier_patch_index: Option<usize>,
    parameter_prop_id_spans_in_first_constructor: Vec<'alloc, Span>,
    parameter_prop_id_spans: Vec<'alloc, Span>,
    parameter_prop_init_insert_start: Option<u32>,
}

#[derive(Debug)]
struct FunctionWithParamPropsScope<'alloc> {
    parameter_prop_id_spans: Vec<'alloc, Span>,
    super_call_stmt_end: Option<u32>,
    last_super_call_expr_span: Option<Span>,
    prologue_scan_state: PrologueScanState,
}

#[derive(Debug)]
struct EnumScope<'alloc> {
    member_names: Vec<'alloc, EnumName<'alloc>>,
}

#[derive(Debug)]
struct NamespaceScope<'alloc> {
    current_stmt_binding_identifiers: Vec<'alloc, &'alloc str>,
    // exported_identifiers: Vec<'alloc, &'alloc str>,
    is_ambient: bool,
}

#[derive(Debug)]
enum ScopeKind<'alloc> {
    Other,
    Class(ClassScope<'alloc>),
    FunctionWithParamProps(FunctionWithParamPropsScope<'alloc>),
    Enum(EnumScope<'alloc>),
    Namespace(NamespaceScope<'alloc>),
}

#[derive(Debug)]
struct EnumName<'alloc> {
    value: &'alloc str,
    is_identifier: bool,
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
            patches: PatchBuilder::new(allocator),
            allocator,
            scope_stack: NonEmptyStack::with_capacity(
                32,
                Scope {
                    last_statement: None,
                    kind: ScopeKind::Other,
                    member_identifiers_by_enum_names: HashMap::new_in(allocator),
                    current_enum_decl: None,
                    current_namespace_decl: None,
                },
            ),
        }
    }
    pub fn scope_len(&self) -> usize {
        self.scope_stack.len() - 1
    }
    pub fn into_patches(self) -> Vec<'alloc, Patch<'alloc>> {
        self.patches.into_patches()
    }

    fn source_bytes(&self) -> &[u8] {
        self.source.as_bytes()
    }

    fn cur_scope(&self) -> &Scope<'alloc> {
        self.scope_stack.last()
    }

    fn cur_scope_mut(&mut self) -> &mut Scope<'alloc> {
        self.scope_stack.last_mut()
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
        let ScopeKind::Class(ClassScope {
            current_element_first_modifier_patch_index,
            ..
        }) = &mut self.scope_stack.last_mut().kind
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
        let first_modifier_patch = &mut self.patches[current_element_first_modifier_patch_index];

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
            if last_patch.replacement.is_empty() && last_patch.span.end == span.end {
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
            is_first,
            patch: current_stmt_patch,
        });
    }

    fn handle_property_paramemter(&mut self, param: &FormalParameter<'_, impl AstAllocator>) {
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

        let current_scope_kind = &mut self.scope_stack.last_mut().kind;
        match current_scope_kind {
            ScopeKind::Other => {
                *current_scope_kind =
                    ScopeKind::FunctionWithParamProps(FunctionWithParamPropsScope {
                        super_call_stmt_end: None,
                        parameter_prop_id_spans: {
                            let mut prop_id_spans = Vec::with_capacity_in(1, self.allocator);
                            prop_id_spans.push(param_id_span);
                            prop_id_spans
                        },
                        last_super_call_expr_span: None,
                        prologue_scan_state: PrologueScanState::Init,
                    })
            }
            ScopeKind::FunctionWithParamProps(FunctionWithParamPropsScope {
                parameter_prop_id_spans,
                ..
            }) => {
                parameter_prop_id_spans.push(param_id_span);
            }
            other => panic!(
                "Formal parameter encountered in unexpected scope: {:?}",
                other
            ),
        };
    }

    fn handle_statement_in_scope_with_param_props(
        stmt: &Statement<'_, impl AstAllocator>,
        scope: &mut FunctionWithParamPropsScope,
    ) {
        match &mut scope.prologue_scan_state {
            PrologueScanState::Init => {
                scope.prologue_scan_state = PrologueScanState::End {
                    last_prologue_stmt_end: None,
                }
            }
            PrologueScanState::InPrologues {
                last_prologue_stmt_end,
            } => {
                if *last_prologue_stmt_end != stmt.span().end {
                    scope.prologue_scan_state = PrologueScanState::End {
                        last_prologue_stmt_end: Some(*last_prologue_stmt_end),
                    }
                }
            }
            PrologueScanState::End { .. } => {}
        }
    }

    fn handle_expr_statement_in_scope_with_param_props(
        expr_stmt: &ExpressionStatement<'_, impl AstAllocator>,
        scope: &mut FunctionWithParamPropsScope<'_>,
    ) {
        // constructor (...) { ... }
        if let (Expression::CallExpression(call_expr), Some(last_super_call_expr_span)) =
            (&expr_stmt.expression, scope.last_super_call_expr_span)
        {
            if call_expr.span() == last_super_call_expr_span {
                scope.super_call_stmt_end = Some(expr_stmt.span.end);
            }
        }
        if let Expression::StringLiteral(_) = expr_stmt.expression {
            if matches!(
                scope.prologue_scan_state,
                PrologueScanState::Init | PrologueScanState::InPrologues { .. }
            ) {
                scope.prologue_scan_state = PrologueScanState::InPrologues {
                    last_prologue_stmt_end: expr_stmt.span.end,
                }
            }
        }
    }
}

impl<'source, 'alloc, 'ast, A: AstAllocator> ParserHandler<'ast, A>
    for StripHandler<'source, 'alloc>
{
    type Checkpoint = StripHandlerCheckpoint;

    fn checkpoint(&self) -> Self::Checkpoint {
        StripHandlerCheckpoint {
            patch_len: self.patches.len() as u32,
            scope_stack_len: self.scope_stack.len() as u32,
        }
    }

    fn rewind(&mut self, checkpoint: Self::Checkpoint) {
        self.patches.truncate(checkpoint.patch_len as usize);

        // TODO: implement NonEmptyStack::truncate
        // self.scope_stack.truncate(checkpoint.scope_stack_len);
        while (checkpoint.scope_stack_len as usize) < self.scope_stack.len() {
            self.scope_stack.pop();
        }
    }
}

impl<'source, 'alloc, 'ast, A: AstAllocator> AstHandler<'ast, A> for StripHandler<'source, 'alloc> {
    fn enter_scope<T: AstScopeNode>(&mut self) {
        let kind = match T::SCOPE_TYPE {
            ScopeType::Class => ScopeKind::Class(ClassScope {
                current_element_first_modifier_patch_index: None,
                parameter_prop_id_spans_in_first_constructor: Vec::new_in(self.allocator),
                parameter_prop_id_spans: Vec::new_in(self.allocator),
                parameter_prop_init_insert_start: None,
            }),
            ScopeType::TSEnumDeclaration => ScopeKind::Enum(EnumScope {
                member_names: Vec::new_in(self.allocator),
            }),
            ScopeType::TSModuleDeclaration => ScopeKind::Namespace(NamespaceScope {
                current_stmt_binding_identifiers: Vec::new_in(self.allocator),
                // exported_identifiers: Vec::new_in(self.allocator),
                is_ambient: true,
            }),
            _ => ScopeKind::Other,
        };
        self.scope_stack.push(Scope {
            last_statement: None,
            kind,
            current_enum_decl: None,
            member_identifiers_by_enum_names: HashMap::new_in(self.allocator),
            current_namespace_decl: None,
        });
    }

    fn leave_scope(&mut self) {
        let scope = self.scope_stack.pop();
        match scope.kind {
            ScopeKind::FunctionWithParamProps(FunctionWithParamPropsScope {
                parameter_prop_id_spans: parameter_prop_id_spans_under_function,
                super_call_stmt_end,
                prologue_scan_state,
                ..
            }) => {
                let Scope {
                    kind:
                        ScopeKind::Class(ClassScope {
                            parameter_prop_id_spans_in_first_constructor,
                            parameter_prop_id_spans: parameter_prop_id_spans_under_class,
                            parameter_prop_init_insert_start,
                            ..
                        }),
                    ..
                } = self.scope_stack.last_mut()
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
                    super_call_stmt_end.or(match prologue_scan_state {
                        PrologueScanState::InPrologues {
                            last_prologue_stmt_end,
                        } => Some(last_prologue_stmt_end),
                        PrologueScanState::End {
                            last_prologue_stmt_end,
                        } => last_prologue_stmt_end,
                        PrologueScanState::Init => None,
                    })
            }
            ScopeKind::Enum(EnumScope { member_names }) => {
                let scope = self.scope_stack.last_mut();
                let enum_name = scope.current_enum_decl.as_ref().unwrap().enum_name;
                let member_identifiers = scope
                    .member_identifiers_by_enum_names
                    .entry(enum_name)
                    .or_insert_with(|| HashSet::new_in(self.allocator));
                member_identifiers.extend(member_names.into_iter().filter_map(|member_name| {
                    if member_name.is_identifier {
                        Some(member_name.value)
                    } else {
                        None
                    }
                }));
            }
            ScopeKind::Namespace(namespace_scope) if !namespace_scope.is_ambient => {
                let scope = self.scope_stack.last_mut();
                scope.current_namespace_decl.as_mut().unwrap().is_ambient = false;
            }
            _ => {}
        }
    }

    fn handle_ts_export_assignment(&mut self, assignment: &TSExportAssignment<'ast, A>) {
        self.patches.binary_search_insert((
            Span::new(assignment.span.start, assignment.expression.span().start),
            "module.exports = ",
        ));
    }

    fn handle_export_specifier(&mut self, specifier: &ExportSpecifier<'ast>) {
        if specifier.export_kind.is_type() {
            self.patches.push(specifier.span);
        }
    }

    fn handle_import_specifier(&mut self, specifier: &ImportSpecifier<'ast>) {
        if specifier.import_kind.is_type() {
            self.patches.push(specifier.span);
        }
    }

    fn handle_ts_namespace_export_declaration(
        &mut self,
        decl: &TSNamespaceExportDeclaration<'ast>,
    ) {
        // export as namespace Foo;
        self.patches.push(decl.span);
    }

    fn handle_export_named_declaration(&mut self, decl: &ExportNamedDeclaration<'ast, A>) {
        if decl.export_kind.is_type() {
            self.patches.push_merging_tail(decl.span);
            return;
        }
        let Some(exported_decl) = &decl.declaration else {
            return;
        };
        if matches!(self.patches.last(), Some(last_patch) if last_patch.replacement.is_empty() && last_patch.span == exported_decl.span())
        {
            self.patches.push_merging_tail(decl.span);
        } else if matches!(exported_decl, Declaration::TSEnumDeclaration(_)) {
            let current_enum_decl = self.scope_stack.last().current_enum_decl.as_ref().unwrap();
            if current_enum_decl.is_secondary {
                self.patches[current_enum_decl.index_of_patch_before_enum_name]
                    .span
                    .start = decl.span.start;
            }
        }
    }

    fn handle_export_default_declaration(&mut self, decl: &ExportDefaultDeclaration<'ast, A>) {
        let Some(last_patch) = self.patches.last() else {
            return;
        };
        if last_patch.span == decl.declaration.span() {
            self.patches.push_merging_tail(decl.span());
        }
    }

    fn handle_export_all_declaration(&mut self, decl: &ExportAllDeclaration<'ast, A>) {
        if decl.export_kind.is_type() {
            self.patches.push_merging_tail(decl.span);
        }
    }

    fn handle_ts_class_implements(&mut self, implements: &TSClassImplements<'ast, A>) {
        self.patches.push_merging_tail(implements.span);
    }

    fn handle_variable_declaration(&mut self, decl: &VariableDeclaration<'ast, A>) {
        if decl.declare {
            self.patches.push_merging_tail(decl.span);
        }
    }

    fn handle_ts_interface_declaration(
        &mut self,
        interface_decl: &TSInterfaceDeclaration<'ast, A>,
    ) {
        self.patches.push_merging_tail(interface_decl.span);
    }

    fn handle_ts_module_declaration_name(&mut self, name: &TSModuleDeclarationName<'ast>) {
        let TSModuleDeclarationName::Identifier(name_identifier) = name else {
            return;
        };
        let cur_scope = self.scope_stack.last_mut();
        let namespace_name = &self.source[name_identifier.span];

        cur_scope.current_namespace_decl = Some(CurrentNamespaceDeclaration {
            namespace_name: self.allocator.alloc_str(namespace_name),
            // The span should cover the namespace/module token, but we don't know the start of it in `handle_ts_module_declaration_name`.
            // Store the index of the patch and change the span start later in `handle_ts_module_declaration``
            index_of_patch_before_namespace_name: self.patches.len(),
            is_ambient: true,
        });

        self.patches.push((
            name_identifier.span.start..name_identifier.span.start,
            format!(in &self.allocator, "var {};(function(", namespace_name).into_bump_str(),
        ));
        self.patches
            .push(((name_identifier.span.end..name_identifier.span.end), "){"));
    }

    fn handle_binding_identifier(&mut self, id: &BindingIdentifier<'ast>) {
        if let ScopeKind::Namespace(NamespaceScope {
            current_stmt_binding_identifiers,
            ..
        }) = &mut self.scope_stack.last_mut().kind
        {
            let id = &self.source[id.span];
            current_stmt_binding_identifiers.push(self.allocator.alloc_str(id));
        }
    }

    fn handle_ts_module_declaration(&mut self, decl: &TSModuleDeclaration<'ast, A>) {
        if decl.declare {
            self.patches.push_merging_tail(decl.span);
            return;
        }

        let Some(current_namespace_decl) = &self.scope_stack.last().current_namespace_decl else {
            return;
        };

        if current_namespace_decl.is_ambient {
            self.patches.push_merging_tail(decl.span);
            return;
        }

        self.patches[current_namespace_decl.index_of_patch_before_namespace_name]
            .span
            .start = decl.span.start;

        // handling namespace A in `namespace A.B`.
        if let Some(TSModuleDeclarationBody::TSModuleDeclaration(inner_module)) = &decl.body {
            // extend the patch after the namepsace name to remove the dot
            self.patches[current_namespace_decl.index_of_patch_before_namespace_name + 1]
                .span
                .end = inner_module.span().start;
        }

        // if the decl starts with the decl id, then we are at namespace B of `namespace A.B`
        let tail_replacement = if decl.span.start == decl.id.span().start {
            let parent_namespace_name = self.scope_stack[self.scope_stack.len() - 2].current_namespace_decl.as_ref().expect("expect parent namespace A to exist while handling a subnamespace B (namespace A.B { .. }) ").namespace_name;
            // }).call(B = A.B || A.B = {}, A.B);
            format!(in &self.allocator, "}}).call({0}={1}.{0}||({1}.{0}={{}}),{1}.{0});", current_namespace_decl.namespace_name, parent_namespace_name)
        } else {
            format!(in &self.allocator, "}}).call({0}||({0}={{}}),{0});", current_namespace_decl.namespace_name)
        };

        self.patches.push((
            (decl.span.end..decl.span.end),
            tail_replacement.into_bump_str(),
        ));

        if let ScopeKind::Namespace(scope) = &mut self.scope_stack.last_mut().kind {
            scope.is_ambient = false;
        }
    }

    fn handle_ts_enum_head(&mut self, enum_head: &TSEnumHead<'ast>) {
        let cur_scope = self.scope_stack.last_mut();
        let enum_name = enum_head.id.name.as_str();
        let existing_member_identifiers = cur_scope.member_identifiers_by_enum_names.get(enum_name);
        let is_secondary = existing_member_identifiers.is_some();

        cur_scope.current_enum_decl = Some(CurrentEnumDeclaration {
            enum_name: self.allocator.alloc_str(enum_name),
            index_of_patch_before_enum_name: self.patches.len(),
            is_secondary,
        });

        // `(const) enum A {` -> `var A;(function(A){var {Foo,Bar}=A;{`
        self.patches
            .push(((enum_head.span.start..enum_head.id.span.start), "var "));
        // self.patches.push_checking_line_terminator(Patch {
        //     span: (enum_head.span.start..enum_head.id.span.start).into(),
        //     replacement: if !is_secondary {
        //         format!(in &self.allocator, "var {};(function(", enum_name).into_bump_str()
        //     } else {
        //         "(function("
        //     },
        // });

        self.patches.push(Patch {
            span: (enum_head.id.span.end..enum_head.id.span.end).into(),
            replacement: {
                let mut replacement = format!(in &self.allocator, ";(function({}){{", enum_name);
                if let Some(existing_member_identifiers) = existing_member_identifiers {
                    if !existing_member_identifiers.is_empty() {
                        replacement.push_str("var {");
                        for (index, member_id) in existing_member_identifiers.iter().enumerate() {
                            replacement.push_str(member_id);
                            if index < existing_member_identifiers.len() - 1 {
                                replacement.push(',');
                            }
                        }
                        replacement.push_str("}=");
                        replacement.push_str(enum_name);
                        replacement.push(';');
                    }
                }
                replacement.into_bump_str()
            },
        });
    }

    fn handle_ts_enum_member_name(&mut self, member_name: &TSEnumMemberName<'ast, A>) {
        let ScopeKind::Enum(EnumScope { member_names }) = &mut self.scope_stack.last_mut().kind
        else {
            if cfg!(debug_assertions) {
                panic!("expect current scope to be Enum when TSEnumMemberName is encountered")
            }
            return;
        };
        let span = member_name.span();
        let name = match member_name {
            TSEnumMemberName::StaticStringLiteral(string_literal) => {
                let name = string_literal.value.as_str();
                if is_identifier_name(name) {
                    EnumName {
                        value: self.allocator.alloc_str(name),
                        is_identifier: true,
                    }
                } else {
                    EnumName {
                        value: self.allocator.alloc_str(&self.source[span]),
                        is_identifier: false,
                    }
                }
            }
            TSEnumMemberName::StaticIdentifier(id) => EnumName {
                value: self.allocator.alloc_str(id.name.as_str()),
                is_identifier: true,
            },
            _ => EnumName {
                value: self.allocator.alloc_str(&self.source[span]),
                is_identifier: false,
            },
        };
        if !name.is_identifier {
            self.patches.push_merging_tail((
                span,
                // this[this["C\n"] = 0] = "C\n";
                // ^^^^^^^^^^^^^^^^
                format!(in &self.allocator, "this[this[{}]", name.value).into_bump_str(),
            ));
        } else if matches!(member_name, TSEnumMemberName::StaticStringLiteral(_)) {
            // "validIdentifier" to validIdentifier
            self.patches.push_merging_tail((span, name.value));
        }
        member_names.push(name);
    }

    fn handle_ts_enum_member(&mut self, member: &TSEnumMember<'ast, A>) {
        let ScopeKind::Enum(EnumScope { member_names }) = &self.scope_stack.last_mut().kind else {
            if cfg!(debug_assertions) {
                panic!("expect current scope to be Enum when TSEnumMember is encountered")
            }
            return;
        };

        let current_member_name = member_names.last().unwrap();
        let mut replacement = String::from_str_in("", self.allocator);

        // init code
        if member.initializer.is_none() {
            replacement.push('=');
            if let Some(last_member_idx) = member_names.len().checked_sub(2) {
                let last_member_name = &member_names[last_member_idx];
                if last_member_name.is_identifier {
                    // = A
                    replacement.push_str(last_member_name.value)
                } else {
                    // = this['A\n']
                    replacement.push_str("this[");
                    replacement.push_str(last_member_name.value);
                    replacement.push(']');
                }
                replacement.push_str("+1");
            } else {
                // = 0
                replacement.push('0');
            };
        };

        if current_member_name.is_identifier {
            // A = 0;var A;this[this.A=A]="A";
            //       ^^^^^^^^^^^^^^^^^^^^^^^^^
            replacement
                .write_fmt(format_args!(
                    ";var {0};this[this.{0}={0}]='{0}';",
                    current_member_name.value
                ))
                .unwrap();
        } else {
            // this[this["C\n"] = 0]="C\n";
            //                     ^^^^^^^^^^
            replacement
                .write_fmt(format_args!("]={};", current_member_name.value))
                .unwrap();
        }

        let end = member.span.end;
        let mut span = Span::new(end, end);
        if self.source.as_bytes().get(end as usize).copied() == Some(b',') {
            span.end += 1;
        }
        self.patches.push((span, replacement.into_bump_str()));
    }

    fn handle_ts_enum_declaration(&mut self, enum_decl: &TSEnumDeclaration<'ast, A>) {
        if enum_decl.head.declare {
            self.patches.push_merging_tail(enum_decl.span);
            return;
        }
        let id = &self.source[enum_decl.head.id.span.range()];

        self.patches.push(Patch {
            span: (enum_decl.span.end..enum_decl.span.end).into(),
            replacement: format!(in &self.allocator, "}}).call({0}||({0}={{}}),{0});", id)
                .into_bump_str(),
        });
    }

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

    fn handle_import_declaration(&mut self, decl: &ImportDeclaration<'ast, A>) {
        if decl.import_kind.is_type() {
            self.patches.push_merging_tail(decl.span);
        }
    }

    fn handle_ts_type_alias_declaration(&mut self, decl: &TSTypeAliasDeclaration<'ast, A>) {
        self.patches.push_merging_tail(decl.span);
    }

    fn handle_function(&mut self, func: &Function<'ast, A>) {
        if func.declare || func.body.is_none() {
            self.patches.push_merging_tail(func.span);
        }
    }

    fn handle_class_element(&mut self, element: &ClassElement<'ast, A>) {
        let span = element.span();
        self.class_element_prefix_patch_asi(span.start);
        let ScopeKind::Class(ClassScope {
            current_element_first_modifier_patch_index,
            ..
        }) = &mut self.scope_stack.last_mut().kind
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

        if let Some(last_patch) = self.patches.last_mut() {
            // Ensure patched tail has `;` to handle code like: `
            // set: any
            // a() { }
            // `
            if last_patch.span.end == span.end && !last_patch.replacement.ends_with(";") {
                last_patch.replacement =
                    format!(in &self.allocator, "{};", last_patch.replacement).into_bump_str();
            }
        }
    }

    fn handle_statement(&mut self, stmt: &Statement<'ast, A>) {
        let scope = self.scope_stack.last_mut();
        scope.current_enum_decl = None;
        scope.current_namespace_decl = None;
        match &mut scope.kind {
            ScopeKind::FunctionWithParamProps(scope) => {
                Self::handle_statement_in_scope_with_param_props(stmt, scope)
            }
            ScopeKind::Namespace(NamespaceScope {
                current_stmt_binding_identifiers,
                is_ambient: ambient,
                ..
            }) => {
                *ambient = *ambient
                    && self
                        .patches
                        .last()
                        .map(|last_patch| {
                            matches!(last_patch.replacement, "" | ";")
                                && last_patch.span == stmt.span()
                        })
                        .unwrap_or(false);
                if let Statement::ExportNamedDeclaration(export_stmt) = stmt {
                    // remove `export` of the export decl in namespaces, and add assignments after the decl...
                    let export_token_start = export_stmt.span().start;
                    // provided the whole decl is not already removed (e.g. export interface/export declare)
                    if !matches!(self.patches.last(), Some(patch) if patch.span.start == export_token_start)
                    {
                        let export_span = Span::new(
                            export_token_start,
                            export_token_start + const { "export".len() as u32 },
                        );
                        debug_assert_eq!(&self.source[export_span], "export");
                        self.patches.binary_search_insert((export_span, ""));

                        let mut assignments = String::new_in(self.allocator);
                        let end = export_stmt.span().end;
                        if self.source.as_bytes()[end as usize - 1] != b';' {
                            assignments.push(';');
                        }
                        for id in current_stmt_binding_identifiers.iter() {
                            assignments
                                .write_fmt(format_args!("this.{0}={0};", id))
                                .unwrap();
                        }
                        self.patches.push(((end..end), assignments.into_bump_str()));
                    }
                }
                current_stmt_binding_identifiers.clear();
            }
            _ => {}
        }
        self.statement_asi(stmt.span());
    }

    #[inline]
    fn handle_expression_statement(&mut self, expr_stmt: &ExpressionStatement<'ast, A>) {
        if let ScopeKind::FunctionWithParamProps(scope) = &mut self.scope_stack.last_mut().kind {
            Self::handle_expr_statement_in_scope_with_param_props(expr_stmt, scope);
        };
    }

    fn handle_call_expression(&mut self, call_expr: &CallExpression<'ast, A>) {
        if matches!(call_expr.callee, Expression::Super(_)) {
            if let ScopeKind::FunctionWithParamProps(FunctionWithParamPropsScope {
                last_super_call_expr_span,
                ..
            }) = &mut self.scope_stack.last_mut().kind
            {
                *last_super_call_expr_span = Some(call_expr.span)
            }
        }
    }

    fn handle_ts_type_annotation(&mut self, it: &TSTypeAnnotation<'ast, A>) {
        self.patches.push_merging_tail(it.span);
    }

    fn handle_ts_type_parameter_declaration(&mut self, it: &TSTypeParameterDeclaration<'ast, A>) {
        self.patches.push_merging_tail(it.span);
    }

    fn handle_ts_type_parameter_instantiation(
        &mut self,
        it: &TSTypeParameterInstantiation<'ast, A>,
    ) {
        self.patches.push_merging_tail(it.span);
    }

    fn handle_ts_as_expression(&mut self, it: &TSAsExpression<'ast, A>) {
        self.patches
            .push_merging_tail(it.expression.span().end..it.span.end);
    }

    fn handle_ts_satisfies_expression(&mut self, it: &TSSatisfiesExpression<'ast, A>) {
        self.patches
            .push_merging_tail(it.expression.span().end..it.span.end);
    }

    fn handle_class_modifiers(&mut self, modifiers: &ClassModifiers) {
        if modifiers.r#abstract {
            self.patches.push_merging_tail(modifiers.span);
        }
    }

    fn handle_class_body(&mut self, class_body: &ClassBody<'ast, A>) {
        let Scope {
            kind:
                ScopeKind::Class(ClassScope {
                    parameter_prop_id_spans_in_first_constructor,
                    ..
                }),
            ..
        } = self.scope_stack.last()
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
            self.allocator,
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

    fn handle_class(&mut self, it: &Class<'ast, A>) {
        if it.modifiers.is_some_and(|modifiers| modifiers.declare) {
            self.patches.push_merging_tail(it.span);
        }
    }

    fn handle_ts_this_parameter(&mut self, it: &TSThisParameter<'ast, A>) {
        self.patches.push_merging_tail(it.span);
    }

    fn handle_ts_function_type(&mut self, ts_func_type: &TSFunctionType<'ast, A>) {
        // ignore param props in function types (`constructor(a: (public b) => void) {}`)
        if let ScopeKind::FunctionWithParamProps(FunctionWithParamPropsScope {
            parameter_prop_id_spans,
            ..
        }) = &mut self.scope_stack.last_mut().kind
        {
            let ts_func_type_start = ts_func_type.span.start;
            while matches!(parameter_prop_id_spans.last(), Some(span) if span.start >= ts_func_type_start)
            {
                parameter_prop_id_spans.pop();
            }
        }
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

        let ScopeKind::Class(ClassScope {
            current_element_first_modifier_patch_index,
            ..
        }) = &mut self.scope_stack.last_mut().kind
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

    fn handle_ts_definite_mark(&mut self, mark: &TSDefiniteMark) {
        self.patches.push_merging_tail(mark.span);
    }

    fn handle_ts_optional_mark(&mut self, mark: &TSOptionalMark) {
        self.patches.push_merging_tail(mark.span);
    }

    fn handle_method_definition(&mut self, element: &MethodDefinition<'ast, A>) {
        if matches!(self.patches.last(), Some(last_patch) if last_patch.span == element.value.span() && last_patch.replacement.is_empty())
        {
            // if the function part is stripped (declare or empty body), strip the whole method
            self.patches.push_merging_tail(element.span);
        }

        if let ScopeKind::Class(ClassScope {
            parameter_prop_id_spans_in_first_constructor,
            parameter_prop_id_spans,
            parameter_prop_init_insert_start,
            ..
        }) = &mut self.scope_stack.last_mut().kind
        {
            if let (MethodDefinitionKind::Constructor, Some(body)) =
                (element.kind, &element.value.body)
            {
                let mut prop_init_code: String<'_> = String::with_capacity_in(
                    parameter_prop_id_spans
                        .iter()
                        .map(|id_span| {
                            "this.".len()
                                + id_span.size() as usize
                                + "=".len()
                                + id_span.size() as usize
                                + ";".len()
                        })
                        .sum::<usize>()
                        + 1,
                    self.allocator,
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
                    prop_init_code.push_str("this.");
                    prop_init_code.push_str(ident);
                    prop_init_code.push_str("=");
                    prop_init_code.push_str(ident);
                    prop_init_code.push_str(";");
                }
                let prop_init_code = prop_init_code.into_bump_str();

                self.patches
                    .binary_search_insert((insert_span, prop_init_code));
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

    fn handle_property_definition(&mut self, element: &PropertyDefinition<'ast, A>) {
        if element
            .modifiers
            .is_some_and(|modifiers| modifiers.declare || modifiers.r#abstract)
        {
            self.patches.push_merging_tail(element.span);
        }
    }

    fn handle_accessor_property(&mut self, element: &AccessorProperty<'ast, A>) {
        if element
            .modifiers
            .is_some_and(|modifiers| modifiers.declare || modifiers.r#abstract)
        {
            self.patches.push_merging_tail(element.span);
        }
    }

    fn handle_ts_index_signature(&mut self, element: &TSIndexSignature<'ast, A>) {
        self.patches.push_merging_tail(element.span);
    }

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

    fn handle_arrow_function_expression(&mut self, arrow_func: &ArrowFunctionExpression<'ast, A>) {
        /*
           `<T>
           () =>`
            to
           `(
           ) =>`
        */
        if let Some(type_param) = &arrow_func.type_parameters {
            let type_param_strip_patch_index = self
                .patches
                .binary_search_by_key(&type_param.span().start, |patch| patch.span.start)
                .unwrap();
            let type_param_strip_patch = &mut self.patches[type_param_strip_patch_index];
            debug_assert_eq!(type_param_strip_patch.span, type_param.span());
            debug_assert_eq!(type_param_strip_patch.replacement, "");

            type_param_strip_patch.replacement = "(";
            type_param_strip_patch.span.end = arrow_func.params.span().start + 1;
        }
        /*
        `(): {
        } => ...`
        to
        `(
        ) => ...`
         */
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

    fn handle_ts_type_assertion_annotation(
        &mut self,
        assertion_annotaion: &TSTypeAssertionAnnotation<'ast, A>,
    ) {
        self.patches
            .push_merging_tail((assertion_annotaion.span, "("));
    }

    fn handle_ts_type_assertion(&mut self, type_assertion: &TSTypeAssertion<'ast, A>) {
        self.patches.push(Patch {
            span: (type_assertion.span.end..type_assertion.span.end).into(),
            replacement: ")",
        });
    }

    fn handle_if_statement(&mut self, if_stmt: &IfStatement<'ast, A>) {
        if let (Some(alternate), Some(last_patch)) = (&if_stmt.alternate, self.patches.last_mut()) {
            if last_patch.span == alternate.span() && last_patch.replacement.is_empty() {
                last_patch.replacement = ";"
            }
        }
        let consequent_span = if_stmt.consequent.span();
        let possible_strip_patch_of_consequent = if if_stmt.alternate.is_none() {
            self.patches.last_mut()
        } else if let Ok(index) = self
            .patches
            .binary_search_by_key(&consequent_span.start, |patch| patch.span.start)
        {
            Some(&mut self.patches[index])
        } else {
            None
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
        let Some(modifiers) = &param.modifiers else {
            return;
        };
        if !(modifiers.r#override || modifiers.readonly || modifiers.accessibility.is_some()) {
            return;
        }
        self.handle_property_paramemter(param);
    }
}
