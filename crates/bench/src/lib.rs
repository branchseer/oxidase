mod oxc_parser;
mod oxidase;
mod swc_fast_ts_strip;

pub use oxc_parser::OxcParser;
pub use oxidase::Oxidase;
pub use swc_fast_ts_strip::SwcFastTsStrip;

use std::cell::RefCell;

pub trait Benchee: Default {
    const NAME: &str;
    type Output;
    fn run(&mut self, source: &mut String) -> Self::Output;
}

pub fn remove_codegen(source: &str) -> String {
    use oxidase_tsc::Tsc;
    thread_local! { static TSC: RefCell<Tsc> = RefCell::new(Tsc::new()) }
    TSC.with_borrow_mut(|tsc| tsc.process_ts(source, true, true))
        .unwrap()
        .ts
}
