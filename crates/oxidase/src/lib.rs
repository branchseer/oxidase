mod patch;
mod handler;

pub use oxc_allocator::Allocator;
pub use oxc_allocator::String;
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::{ParseOptions, Parser};
use oxc_span::ast_alloc::AstAllocator;
use oxc_span::ast_alloc::VoidAllocator;
pub use oxc_span::SourceType;
use patch::apply_patches;
use handler::StripHandler;

#[derive(Debug)]
pub struct TranspileReturn {
    pub parser_panicked: bool,
    pub parser_errors: std::vec::Vec<OxcDiagnostic>,
}

pub fn transpile(
    allocator: &Allocator,
    source_type: SourceType,
    source: &mut String<'_>,
) -> TranspileReturn {
    transpile_with_ast_allocator(allocator, &VoidAllocator::new(), source_type, source)
}

#[cfg(feature = "unstable-bench")]
pub fn transpile_allocated(
    allocator: &Allocator,
    source_type: SourceType,
    source: &mut String<'_>,
) -> TranspileReturn {
    transpile_with_ast_allocator(allocator, allocator, source_type, source)
}

fn transpile_with_ast_allocator<A: AstAllocator>(
    allocator: &Allocator,
    ast_allocator: &A,
    source_type: SourceType,
    source: &mut String<'_>,
) -> TranspileReturn {
    let mut parser_options = ParseOptions::default();
    parser_options.allow_skip_ambient = true;
    let parser = Parser::new(allocator, source.as_str(), source_type).with_options(parser_options);
    let handler = StripHandler::new(allocator, source.as_str());

    let mut parser_ret = parser.parse_with_handler(ast_allocator, handler);
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
