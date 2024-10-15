use std::process::id;

use crate::patch::Patch;
use oxc_allocator::{Allocator, Vec};
use oxc_ast::ast::*;
use oxc_ast::visit::walk;
use oxc_ast::Visit;
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

pub struct Visitor<'source, 'alloc> {
    source: &'source str,
    patches: Vec<'alloc, Patch<'alloc>>,
    allocator: &'alloc Allocator,
}
impl<'source, 'alloc> Visitor<'source, 'alloc> {
    pub fn new(allocator: &'alloc Allocator, source: &'source str) -> Self {
        Self {
            source,
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
    fn should_insert_semicolon(&self, is_first: bool, stmt_after: Option<&Statement<'_>>) -> bool {
        if is_first {
            return false;
        }
        let Some(stmt_after) = stmt_after else {
            return false;
        };
        let first_char_of_stmt_after = self.source.as_bytes()[stmt_after.span().start as usize];
        // https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#sec-asi-interesting-cases-in-statement-lists
        matches!(
            first_char_of_stmt_after,
            b'(' | b'[' | b'`' | b'+' | b'-' | b'/'
        )
    }
}

fn should_strip_statement(stmt: &Statement<'_>) -> bool {
    match stmt {
        Statement::TSEnumDeclaration(decl) => decl.declare,
        Statement::TSImportEqualsDeclaration(decl) => decl.import_kind.is_type(),
        Statement::TSInterfaceDeclaration(_) => true,
        Statement::TSModuleDeclaration(decl) => decl.declare,
        Statement::TSTypeAliasDeclaration(_) => true,
        Statement::FunctionDeclaration(decl) => decl.body.is_none(),
        Statement::ClassDeclaration(decl) => decl.declare,
        Statement::ExportAllDeclaration(decl) => decl.export_kind.is_type(),
        Statement::ExportNamedDeclaration(decl) => decl.export_kind.is_type(),
        Statement::ImportDeclaration(decl) => decl.import_kind.is_type(),
        _ => false,
    }
}

impl<'source, 'alloc, 'ast> Visit<'ast> for Visitor<'source, 'alloc> {
    fn visit_statements(&mut self, it: &Vec<'ast, Statement<'ast>>) {
        for (i, stmt) in it.iter().enumerate() {
            if should_strip_statement(stmt) {
                let should_insert_semicolon = self.should_insert_semicolon(i == 0, it.get(i + 1));
                self.patches.push(Patch {
                    span: stmt.span(),
                    replacement: if should_insert_semicolon { ";" } else { "" },
                });
            } else {
                walk::walk_statement(self, stmt);
                let Some(last_patch) = self.patches.last_mut() else {
                    continue;
                };
                if stmt.span().end == last_patch.span.end && !last_patch.replacement.ends_with(';')
                {
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
        }
    }
    fn visit_ts_type_annotation(&mut self, it: &TSTypeAnnotation<'ast>) {
        self.push_strip(it.span);
    }
    fn visit_ts_type_parameter_declaration(&mut self, it: &TSTypeParameterDeclaration<'ast>) {
        self.push_strip(it.span);
    }
    fn visit_ts_type_parameter_instantiation(&mut self, it: &TSTypeParameterInstantiation<'ast>) {
        self.push_strip(it.span);
    }
    fn visit_ts_as_expression(&mut self, it: &TSAsExpression<'ast>) {
        self.push_strip((it.expression.span().end..it.span.end).into());
    }
    fn visit_ts_satisfies_expression(&mut self, it: &TSSatisfiesExpression<'ast>) {
        self.push_strip((it.expression.span().end..it.span.end).into());
    }
    fn visit_class(&mut self, it: &Class<'ast>) {
        if it.r#abstract {
            let mut idx = it
                .decorators
                .last()
                .map(|dcrt| dcrt.span.end)
                .unwrap_or(it.span.start) as usize;
            let source = self.source.as_bytes();
            loop {
                if source[idx..].starts_with(b"class") {
                    break;
                }
                if source[idx..].starts_with(b"abstract") {
                    let start = idx;
                    idx += b"abstract".len();
                    idx += skip_whitespaces(&source[idx..]);
                    self.push_strip((start as u32..idx as u32).into());
                } else {
                    idx += 1;
                }
            }
        }

        walk::walk_class(self, it);
    }
    fn visit_ts_this_parameter(&mut self, it: &TSThisParameter<'ast>) {
        self.push_strip(it.span);
    }
    // fn visit_class_elements(&mut self, it: &Vec<'ast, ClassElement<'ast>>) {
    //     for elem in it {}
    // }
}
