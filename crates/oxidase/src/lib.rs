mod handler;
mod patch;
mod string_buf;

#[doc(hidden)]
pub mod line_term;
mod patch_builder;

use handler::StripHandler;
pub use oxc_allocator::Allocator;
pub use oxc_allocator::String;
pub use oxc_diagnostics;
use oxc_parser::{ParseOptions, Parser};
pub use oxc_span::SourceType;
// expose for bench
#[doc(hidden)]
pub use oxc_span::ast_alloc::VoidAllocator;
#[doc(hidden)]
pub use patch::{apply_patches, Patch};
pub use string_buf::StringBuf;

#[derive(Debug)]
pub struct TranspileReturn {
    pub parser_panicked: bool,
    pub parser_errors: std::vec::Vec<oxc_diagnostics::OxcDiagnostic>,
}

pub fn transpile<S: StringBuf>(
    allocator: &Allocator,
    source_type: SourceType,
    source: &mut S,
) -> TranspileReturn {
    // we are here to transpile, not validate. Be as loose as possible.
    let parser_options = ParseOptions {
        allow_return_outside_function: true,
        allow_skip_ambient: true,
        ..Default::default()
    };

    let parser = Parser::new(allocator, source.as_ref(), source_type).with_options(parser_options);
    let handler = StripHandler::new(allocator, source.as_ref());

    const VOID_ALLOCATOR: VoidAllocator = VoidAllocator::new();
    let mut parser_ret = parser.parse_with(&VOID_ALLOCATOR, handler);
    if parser_ret.panicked {
        return TranspileReturn {
            parser_panicked: true,
            parser_errors: parser_ret.errors,
        };
    }
    let errors = std::mem::take(&mut parser_ret.errors);

    let handler = parser_ret.handler;
    debug_assert_eq!(handler.scope_len(), 0);

    let patches = handler.into_patches();

    unsafe { apply_patches(&patches, source) };

    TranspileReturn {
        parser_panicked: false,
        parser_errors: errors,
    }
}
