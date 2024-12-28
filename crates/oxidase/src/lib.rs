mod handler;
mod patch;


use handler::StripHandler;
pub use oxc_allocator::Allocator;
pub use oxc_allocator::String;
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::{ParseOptions, Parser};
use oxc_span::ast_alloc::AstAllocator;
pub use oxc_span::SourceType;

 // expose for bench 
#[doc(hidden)]
pub use patch::{apply_patches, Patch};
#[doc(hidden)]
pub use oxc_span::ast_alloc::VoidAllocator;

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
    transpile_with_options(allocator, &VoidAllocator::new(), true, source_type, source)
}

#[doc(hidden)] // expose options for bench
pub fn transpile_with_options<A: AstAllocator>(
    allocator: &Allocator,
    ast_allocator: &A,
    allow_skip_ambient: bool,
    source_type: SourceType,
    source: &mut String<'_>,
) -> TranspileReturn {
    let mut parser_options = ParseOptions::default();
    // we are here to transpile, not validate. Be as loose as possible.
    parser_options.allow_return_outside_function = true;
    parser_options.allow_skip_ambient = allow_skip_ambient;
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
