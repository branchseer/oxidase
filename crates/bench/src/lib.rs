mod oxc_parser;
mod oxidase;
mod swc_fast_ts_strip;

#[cfg(feature = "wasm")]
mod wasm;

pub use oxc_parser::OxcParser;
pub use oxidase::Oxidase;
pub use swc_fast_ts_strip::SwcFastTsStrip;

pub trait Benchee: Default {
    const NAME: &str;
    type Output;
    fn run(&mut self, source: &mut String) -> Self::Output;
}
