mod patch;
mod handler;

pub use oxc_allocator::Allocator;
pub use oxc_allocator::String;
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::{ParseOptions, Parser};
pub use oxc_span::SourceType;
use oxc_span::ModuleKind as OxcModuleKind;
use patch::apply_patches;
use handler::StripHandler;

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


#[derive(Debug)]
pub struct TranspileReturn {
    pub parser_panicked: bool,
    pub parser_errors: std::vec::Vec<OxcDiagnostic>,
}

pub fn transpile<'alloc>(
    allocator: &Allocator,
    source_type: SourceType,
    source: &mut String<'_>,
) -> TranspileReturn {
    let mut parser_options = ParseOptions::default();
    parser_options.allow_skip_ambient = true;
    let parser = Parser::new(allocator, source.as_str(), source_type).with_options(parser_options);
    let handler = StripHandler::new(allocator, source.as_str());

    let mut parser_ret = parser.parse_with_handler(handler);
    if parser_ret.panicked {
        return TranspileReturn {
            parser_panicked: true,
            parser_errors: parser_ret.errors,
        };
    }
    let errors = std::mem::take(&mut parser_ret.errors);

    let mut patches = parser_ret.handler.into_patches();

    apply_patches(&mut patches, source);

    TranspileReturn {
        parser_panicked: false,
        parser_errors: errors,
    }
}
