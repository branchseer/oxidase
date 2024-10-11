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
use patch::Patch;
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

pub struct Error {
    oxc_errors: std::vec::Vec<OxcDiagnostic>,
}

struct OxcResult<T> {
    ok: Option<T>,
    errors: std::vec::Vec<OxcDiagnostic>,
}

fn transiple_patches<'alloc, 'source>(
    allocator: &'alloc Allocator,
    source_type: SourceType,
    source: &'source str,
) -> OxcResult<Vec<'alloc, Patch<'alloc>>> {
    let parser = ParserWithOpt::<true>::new(allocator, source, source_type);
    let ret = parser.parse();
    if ret.panicked {
        return OxcResult {
            ok: None,
            errors: ret.errors,
        };
    }
    let mut visitor = Visitor::new(allocator);
    oxc_ast::visit::walk::walk_program(&mut visitor, &ret.program);
    OxcResult {
        ok: Some(visitor.into_patches()),
        errors: ret.errors,
    }
}
