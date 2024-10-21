use crate::patch::Patch;
use oxc_allocator::{Allocator, Vec};
use oxc_ast::ast::*;
use oxc_ast::handle::{Handler as AstHandler};
use oxc_parser::Handler as ParserHandler;
use oxc_span::ast_alloc::VoidAllocator;
use oxc_span::GetSpan;

const WHITESPACES: [&str; 25] = [
    // https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#sec-white-space
    "\u{0009}", "\u{000B}", "\u{000C}", "\n{FEFF}", "\u{0020}", "\u{00A0}", "\u{1680}", "\u{2000}",
    "\u{2001}", "\u{2002}", "\u{2003}", "\u{2004}", "\u{2005}", "\u{2006}", "\u{2007}", "\u{2008}",
    "\u{2009}", "\u{200A}", "\u{202F}", "\u{205F}", "\u{3000}",
    // https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#sec-line-terminators
    "\u{000A}", "\u{000D}", "\u{2028}", "\u{2029}",
];

fn skip_whitespaces(source: &[u8]) -> usize {
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

pub struct StripHandler<'source, 'alloc> {
    source: &'source str,
    allocator: &'alloc Allocator,

    patches: Vec<'alloc, Patch<'alloc>>,
    scope_stack: Vec<'alloc, Scope>,
}

#[derive(Clone, Copy, Debug)]
pub struct StripHandlerCheckpoint {
    patch_len: usize,
    scope_stack_len: usize
}

struct LastStatement {
    span: Span,
    is_first: bool,
    strip_patch_index: Option<usize>,
}

#[derive(Default)]
struct Scope {
    last_statement: Option<LastStatement>,
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
    fn push_strip_with_right_whitespaces(&mut self, mut span: Span) {
        self.expand_right_whitespace(&mut span);
        self.push_strip(span);
    }
    fn push_patch(&mut self, span: Span, replacement: &'alloc str) {
        #[cfg(test)]
        assert!(span.end >= self.patches.last().map(|patch| patch.span.end).unwrap_or(0));
        while matches!(self.patches.last(), Some(patch) if patch.span.start >= span.start) {
            self.patches.pop();
        }
        self.patches.push(Patch {
            span,
            replacement,
        });
    }
    // fn insert_patch(&mut self, span: Span, replacement: &'alloc str) {
    //     let mut insert_pos = self.patches.len();
    //     while insert_pos > 0 && self.patches[insert_pos - 1].span.end > span.start {
    //         insert_pos -= 1
    //     };
    //     self.
    // }
    // fn insert_strip(&mut self, span: Span) {
    //     self.insert_patch(span, "");
    // }

    fn source_bytes(&self) -> &[u8] {
        self.source.as_bytes()
    }

    fn cur_scope(&self) -> &Scope {
        self.scope_stack.last().unwrap()
    }
    fn cur_scope_mut(&mut self) -> &mut Scope {
        self.scope_stack.last_mut().unwrap()
    }
    fn expand_right_whitespace(&self, span: &mut Span) {
        let whitespace_len = skip_whitespaces(&self.source.as_bytes()[span.end as usize..]);
        span.end += whitespace_len as u32
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
    fn enter_scope(&mut self) {
        self.scope_stack.push(Scope::default());
    }
    fn leave_scope(&mut self) {
        let scope = self.scope_stack.pop();
        if cfg!(test) {
            scope.unwrap();
        }
    }

    fn handle_ts_interface_declaration(&mut self, interface_decl: &TSInterfaceDeclaration<'ast, VoidAllocator>) {
        self.push_strip(interface_decl.span);
    }
    fn handle_ts_enum_declaration(&mut self, enum_decl: &TSEnumDeclaration<'ast, VoidAllocator>) {
        if enum_decl.declare {
            self.push_strip(enum_decl.span);
        }
    }
    fn handle_ts_import_equals_declaration(&mut self, decl: &TSImportEqualsDeclaration<'ast, VoidAllocator>) {
        if decl.import_kind.is_type() {
            self.push_strip(decl.span);
        }
    }
    fn handle_ts_module_declaration(&mut self, decl: &TSModuleDeclaration<'ast, VoidAllocator>) {
        if decl.declare {
            self.push_strip(decl.span);
        }
    }
    fn handle_ts_type_alias_declaration(&mut self, decl: &TSTypeAliasDeclaration<'ast, VoidAllocator>) {
        if decl.declare {
            self.push_strip(decl.span);
        }
    }
    fn handle_function(&mut self, func: &Function<'ast, VoidAllocator>) {
        if func.declare || func.body.is_none() {
            self.push_strip(func.span);
        }
    }

    fn handle_statement(&mut self, stmt: &Statement<'ast, VoidAllocator>) {
        let span = stmt.span();
        let mut is_first = true;
        if let Some(last_statement) = &self.cur_scope().last_statement {
            is_first = false;
            if let (
                Some(strip_patch_index),
                false,
                b'(' | b'[' | b'`' | b'+' | b'-' | b'/'
            ) = (
                last_statement.strip_patch_index,
                last_statement.is_first,
                self.source.as_bytes()[span.start as usize]
            ) {
                self.patches.as_mut_slice()[strip_patch_index].replacement = ";"
            }
        }
        let mut strip_patch_index = None;
        if let Some(last_patch) = self.patches.last_mut() {
            if last_patch.span == span {
                strip_patch_index = Some(self.patches.len() - 1)
            } else if last_patch.span.end == span.end {
                if last_patch.replacement.is_empty() {
                    last_patch.replacement = ";"
                } else {
                    let insert_span = Span::from(last_patch.span.end..last_patch.span.end);
                    self.patches.push(Patch {
                        span: insert_span,
                        replacement: ";",
                    })
                }
            }
        }
        self.cur_scope_mut().last_statement = Some(LastStatement { span, is_first, strip_patch_index });
    }

    fn handle_ts_type_annotation(&mut self, it: &TSTypeAnnotation<'ast, VoidAllocator>) {
        self.push_strip(it.span);
    }
    fn handle_ts_type_parameter_declaration(&mut self, it: &TSTypeParameterDeclaration<'ast, VoidAllocator>) {
        self.push_strip(it.span);
    }
    fn handle_ts_type_parameter_instantiation(&mut self, it: &TSTypeParameterInstantiation<'ast, VoidAllocator>) {
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
        if it.modifiers.declare {
            self.push_strip(it.span);
            return;
        }
    }
    fn handle_ts_this_parameter(&mut self, it: &TSThisParameter<'ast, VoidAllocator>) {
        let whitespace_len = skip_whitespaces(&self.source.as_bytes()[it.span.end as usize..]);
        let mut end = it.span.end as usize + whitespace_len;
        if self.source_bytes().get(end).copied() == Some(b',') {
            end += 1
        }
        
        self.push_strip(Span::new(it.span.start, end as u32));
    }
    fn handle_ts_type_assertion_annotation(&mut self, annotation: &TSTypeAssertionAnnotation<'ast, VoidAllocator>) {
        self.push_strip(annotation.span);
    }
    fn handle_class_element_modifiers(&mut self, modifiers: &ClassElementModifiers) {
        if !(modifiers.r#abstract || modifiers.declare || modifiers.r#override || modifiers.readonly || modifiers.accessibility.is_some()) {
            return;
        }
        let replacement = match (modifiers.r#static, modifiers.r#async) {
            (true, true) => "static async ",
            (true, false) => "static ",
            (false, true) => "async ",
            (false, false) => ""
        };
        let mut span = modifiers.span;
        self.expand_right_whitespace(&mut span);
        self.push_patch(span, &replacement);
        return;
    }

    fn handle_ts_definite_mark(&mut self, mark: &TSDefiniteMark) {
        self.push_strip(mark.span);
    }
    fn handle_ts_optional_mark(&mut self, mark: &TSOptionalMark) {
        self.push_strip(mark.span);
    }
}
