use std::{hint::black_box, mem::forget};

use super::Benchee;
use oxc_parser::Parser;
use oxc_span::SourceType;

#[derive(Default)]
pub struct OxcParser {
    allocator: oxc_allocator::Allocator,
}

impl Benchee for OxcParser {
    const NAME: &str = "oxc_parser";
    fn run(&mut self, source: &mut String) {
        self.allocator.reset();
        let ret = Parser::new(&self.allocator, source, SourceType::ts()).parse();
        assert!(ret.errors.is_empty());
        // https://github.com/oxc-project/oxc/pull/6623
        forget(black_box(ret.program));
    }
}
