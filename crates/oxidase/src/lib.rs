mod patch;
mod source;
mod visitor;

use std::cell::Cell;
use std::convert::Infallible;

pub use oxc_allocator::Allocator;
use oxc_allocator::Vec;
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::ParserWithOpt;
pub use oxc_span::SourceType;
use oxc_span::{ModuleKind as OxcModuleKind, Span};
use patch::{apply_patches, Patch};
pub use source::Source;
use visitor::Visitor;

type HashMap<'a, K, V> = hashbrown::HashMap<K, V, rustc_hash::FxBuildHasher, &'a bumpalo::Bump>;
type HashSet<'a, T> = hashbrown::HashSet<T, rustc_hash::FxBuildHasher, &'a bumpalo::Bump>;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ModuleKind {
    /// Regular JS script or CommonJS file
    Script,
    /// ES6 Module
    Module,
}
impl From<ModuleKind> for OxcModuleKind {
    fn from(value: ModuleKind) -> Self {
        match value {
            ModuleKind::Script => OxcModuleKind::Script,
            ModuleKind::Module => OxcModuleKind::Module,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TranspileOptions {
    pub source_type: SourceType,
    pub prefer_blank_space: bool,
}

#[derive(Debug)]
pub struct TranspileReturn {
    pub panicked: bool,
    pub errors: std::vec::Vec<OxcDiagnostic>,
}

pub fn transpile<'alloc>(
    allocator: &'alloc Allocator,
    options: TranspileOptions,
    source: &mut Source<'_, 'alloc>,
) -> TranspileReturn {
    let parser = ParserWithOpt::<true>::new(allocator, source.as_str(), options.source_type);
    let mut parser_ret = parser.parse();
    if parser_ret.panicked {
        return TranspileReturn {
            panicked: true,
            errors: parser_ret.errors,
        };
    }
    let errors = std::mem::take(&mut parser_ret.errors);

    let mut visitor = Visitor::new(allocator, source.as_str());
    oxc_ast::visit::walk::walk_program(&mut visitor, &parser_ret.program);

    let patches = visitor.into_patches();

    drop(parser_ret);

    apply_patches(allocator, &patches, options.prefer_blank_space, source);

    TranspileReturn {
        panicked: false,
        errors: errors,
    }
}
