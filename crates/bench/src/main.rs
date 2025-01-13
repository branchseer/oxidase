#![recursion_limit = "256"]

use std::{
    fs::read_to_string, hint::black_box, path::{Path, PathBuf}, sync::Arc
};

use oxidase::{transpile, Allocator, SourceType, String};

fn main() {
    let allocator = Allocator::default();
    let mut source = String::from_str_in( 
        include_str!("/Users/patr0nus/code/oxidase/crates/e2e/fixture/ecosystem/TypeScript/src/compiler/checker.ts")
        // include_str!("/Users/patr0nus/Desktop/parser.ts")
        , &allocator);
    dbg!(source.len());
    transpile(&allocator, SourceType::ts(), &mut source);
    // black_box(source);
    dbg!(source.len());
}
