use std::{
    hint::black_box,
    mem::{transmute, ManuallyDrop},
};

use super::Benchee;
use oxc_ast::ast::Program;
use oxc_diagnostics::OxcDiagnostic;
use oxc_parser::Parser;
use oxc_span::{SourceType, Span};

#[derive(Default)]
pub struct OxcParser {
    allocator: oxc_allocator::Allocator,
}

impl Benchee for OxcParser {
    const NAME: &str = "oxc_parser";
    type Output = (
        Vec<OxcDiagnostic>,
        Box<[Span]>,
        ManuallyDrop<Program<'static>>,
    );
    fn run(&mut self, source: &mut String) -> Self::Output {
        self.allocator.reset();
        let ret = Parser::new(&self.allocator, source, SourceType::ts()).parse();
        assert!(ret.errors.is_empty());
        (
            ret.errors,
            ret.irregular_whitespaces,
            ManuallyDrop::new(unsafe { transmute::<Program<'_>, Program<'static>>(ret.program) }),
        )
    }
}
