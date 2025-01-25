use oxidase::SourceType;

use crate::Benchee;

#[derive(Default)]
pub struct Oxidase {
    allocator: oxidase::Allocator,
}

impl Benchee for Oxidase {
    const NAME: &str = "oxidase";
    type Output = String;
    fn run(&mut self, source: &mut String) -> String {
        self.allocator.reset();
        let ret = oxidase::transpile(&self.allocator, SourceType::ts(), source);
        assert!(ret.parser_errors.is_empty());
        std::mem::take(source)
    }
}
