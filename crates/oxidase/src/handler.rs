use std::ops::Range;

use crate::patch::Patch;
use bumpalo::format;
use oxc_allocator::{Allocator, String, Vec};
use oxc_ast::handle::Handler as AstHandler;
use oxc_ast::{ast::*, AstScopeNode, ScopeType};
use oxc_parser::Handler as ParserHandler;
use oxc_span::ast_alloc::{AstAllocator, VoidAllocator};
use oxc_span::{GetSpan, SPAN};

const WHITESPACES: [&str; 25] = [
    // https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#sec-white-space
    "\u{0009}", "\u{000B}", "\u{000C}", "\n{FEFF}", "\u{0020}", "\u{00A0}", "\u{1680}", "\u{2000}",
    "\u{2001}", "\u{2002}", "\u{2003}", "\u{2004}", "\u{2005}", "\u{2006}", "\u{2007}", "\u{2008}",
    "\u{2009}", "\u{200A}", "\u{202F}", "\u{205F}", "\u{3000}",
    // https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#sec-line-terminators
    "\u{000A}", "\u{000D}", "\u{2028}", "\u{2029}",
];

fn skip_whitespace(source: &[u8]) -> usize {
    let mut idx = 0;
    'outer: while idx < source.len() {
        for ws in WHITESPACES {
            if source[idx..].starts_with(ws.as_bytes()) {
                idx += ws.len();
                continue 'outer;
            }
        }
        break;
    }
    idx
}

trait SpanExt {
    fn range(self) -> Range<usize>;
}
impl SpanExt for Span {
    fn range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

fn skip_ending_whitespace(source: &[u8]) -> usize {
    let mut idx = source.len();
    'outer: while idx >= 0 {
        for ws in WHITESPACES {
            if source[..idx].ends_with(ws.as_bytes()) {
                idx -= ws.len();
                continue 'outer;
            }
        }
        break;
    }
    idx
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_skip_ending_whitespace() {
        assert_eq!(skip_ending_whitespace(b"abc    "), 3);
        assert_eq!(skip_ending_whitespace(b"a"), 1);
        assert_eq!(skip_ending_whitespace(b""), 0);
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
        current_element_modifiers_patch_index: Option<usize>,
    },
    ConstructorWithParamProps {
        patch_index_at_start_of_body: Option<usize>,
        parameter_prop_id_spans: Vec<'alloc, Span>,
        super_call_finding_state: SuperCallFindingState,
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
    fn push_strip_with_right_whitespaces(&mut self, span: Span) {
        self.push_strip(self.expand_right_whitespaces(span));
    }
    fn push_patch(&mut self, span: Span, replacement: &'alloc str) {
        debug_assert!(span.end >= self.patches.last().map(|patch| patch.span.end).unwrap_or(0));

        while matches!(self.patches.last(), Some(patch) if patch.span.start >= span.start) {
            self.patches.pop();
        }
        self.patches.push(Patch { span, replacement });
    }
    fn insert_patch(&mut self, span: Span, replacement: &'alloc str) {
        let mut insert_pos = self.patches.len();
        while insert_pos > 0 && self.patches[insert_pos - 1].span.end > span.start {
            insert_pos -= 1
        }
        #[cfg(debug_assertions)]
        if let Some(patch_after) = self.patches.get(insert_pos) {
            assert!(span.end <= patch_after.span.start);
        }
        self.patches.insert(insert_pos, Patch { span, replacement });
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
    fn expand_right_whitespaces(&self, span: Span) -> Span {
        let whitespace_len = skip_whitespace(&self.source_bytes()[span.end as usize..]);
        Span::new(span.start, span.end + whitespace_len as u32)
    }
    fn expand_left_whitespaces(&self, span: Span) -> Span {
        let left_whitespace_start =
            skip_ending_whitespace(&self.source_bytes()[..span.start as usize]);
        Span::new(left_whitespace_start as u32, span.end)
    }

    fn strip_specifier(&mut self, span: Span) {
        let mut span_to_strip = self.expand_right_whitespaces(span);
        if self.source_bytes().get(span_to_strip.end as usize).copied() == Some(b',') {
            span_to_strip.end += 1;
            span_to_strip = self.expand_left_whitespaces(span_to_strip);
        }
        self.push_strip(span_to_strip);
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
            current_element_modifiers_patch_index,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
        else {
            if cfg!(test) {
                unreachable!();
            }
            return;
        };
        let Some(current_element_modifiers_patch_index) = *current_element_modifiers_patch_index
        else {
            return;
        };
        let modifiers_patch =
            &mut self.patches.as_mut_slice()[current_element_modifiers_patch_index];
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
                current_element_modifiers_patch_index: None,
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

    fn handle_call_expression(&mut self, expr: &CallExpression<'ast, VoidAllocator>) {
        let ScopeKind::ConstructorWithParamProps {
            super_call_finding_state,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
        else {
            return;
        };
        if !matches!(super_call_finding_state, SuperCallFindingState::NotFound) {
            return;
        }
        if let Expression::Super(_) = &expr.callee {
            *super_call_finding_state = SuperCallFindingState::ExpressionFound(expr.span)
        };
    }

    fn handle_export_specifier(&mut self, specifier: &ExportSpecifier<'ast>) {
        if specifier.export_kind.is_type() {
            self.strip_specifier(specifier.span);
        }
    }
    fn handle_import_specifier(&mut self, specifier: &ImportSpecifier<'ast>) {
        if specifier.import_kind.is_type() {
            self.strip_specifier(specifier.span);
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
        self.push_strip_with_right_whitespaces(implements.span);
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
            patch_index_at_start_of_body,
            parameter_prop_id_spans,
            super_call_finding_state,
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
                .sum(),
            &self.allocator,
        );
        for id_span in parameter_prop_id_spans {
            let ident = &self.source[*id_span];
            prop_init_code.push_str(" this.");
            prop_init_code.push_str(ident);
            prop_init_code.push_str(" = ");
            prop_init_code.push_str(ident);
            prop_init_code.push_str(";");
        }
        let prop_init_code = prop_init_code.into_bump_str();
        let body_start = body.span().start;
        debug_assert_eq!(self.source_bytes()[body_start as usize], b'{');
        let insert_span = Span::new(body_start + 1, body_start + 1);
        if let Some(patch_index_at_start_of_body) = patch_index_at_start_of_body {
            let patch = &mut self.patches.as_mut_slice()[*patch_index_at_start_of_body];
            patch.span = insert_span;
            patch.replacement = prop_init_code;
        } else {
            self.patches.push(Patch {
                span: insert_span,
                replacement: prop_init_code,
            });
        }
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
            current_element_modifiers_patch_index,
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
        *current_element_modifiers_patch_index = None;
    }

    fn handle_statement(&mut self, stmt: &Statement<'ast, VoidAllocator>) {
        let stmt_span = stmt.span();
        self.statement_asi(stmt_span);

        let ScopeKind::ConstructorWithParamProps {
            super_call_finding_state,
            patch_index_at_start_of_body,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
        else {
            return;
        };
        if patch_index_at_start_of_body.is_none() {
            *patch_index_at_start_of_body = Some(self.patches.len());
            self.patches.push(Patch {
                span: SPAN,
                replacement: "",
            });
        }
        let SuperCallFindingState::ExpressionFound(super_call_expr_span) =
            &super_call_finding_state
        else {
            return;
        };
        let Statement::ExpressionStatement(expr_stmt) = &stmt else {
            return;
        };
        if *super_call_expr_span == expr_stmt.span() {
            *super_call_finding_state = SuperCallFindingState::StatementFound {
                patch_index_after_stmt: self.patches.len(),
            };
            self.patches.push(Patch {
                span: Span::new(stmt_span.end, stmt_span.end),
                replacement: "",
            });
        } else {
            *super_call_finding_state = SuperCallFindingState::NotFound;
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
            self.push_strip_with_right_whitespaces(modifiers.span);
        }
    }
    fn handle_class(&mut self, it: &Class<'ast, VoidAllocator>) {
        if it.modifiers.is_some_and(|modifiers| modifiers.declare) {
            self.push_strip(it.span);
            return;
        }
    }
    fn handle_ts_this_parameter(&mut self, it: &TSThisParameter<'ast, VoidAllocator>) {
        let whitespace_len = skip_whitespace(&self.source.as_bytes()[it.span.end as usize..]);
        let mut end = it.span.end as usize + whitespace_len;
        if self.source_bytes().get(end).copied() == Some(b',') {
            end += 1
        }

        self.push_strip(Span::new(it.span.start, end as u32));
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
        let replacement = match (modifiers.r#static, modifiers.r#async) {
            (true, true) => "static async ",
            (true, false) => "static ",
            (false, true) => "async ",
            (false, false) => "",
        };
        let span = if self.source_bytes()
            [modifiers.span.start as usize..modifiers.span.end as usize]
            .ends_with(b"accessor")
        {
            // TODO: fix on the parser side to exclude accessor from the modifiers
            modifiers.span.shrink_right(b"accessor".len() as u32)
        } else {
            self.expand_right_whitespaces(modifiers.span)
        };
        self.push_patch(span, replacement);
        let ScopeKind::Class {
            current_element_modifiers_patch_index,
            ..
        } = &mut self.scope_stack.last_mut().unwrap().kind
        else {
            if cfg!(test) {
                panic!(
                    "Class element modifiers encountered in non-class scope: {:?}",
                    modifiers.span
                );
            }
            return;
        };
        *current_element_modifiers_patch_index = Some(self.patches.len() - 1);
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
        self.push_strip(self.expand_right_whitespaces(modifiers.span));
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
                    patch_index_at_start_of_body: None,
                    super_call_finding_state: SuperCallFindingState::NotFound,
                    parameter_prop_id_spans: {
                        let mut prop_id_spans = Vec::with_capacity_in(1, &self.allocator);
                        prop_id_spans.push(param_id_span);
                        prop_id_spans
                    },
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
