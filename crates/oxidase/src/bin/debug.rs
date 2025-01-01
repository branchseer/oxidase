#![recursion_limit = "256"]

use std::{fs::read_to_string, process::abort};

use oxidase::{transpile, Allocator, SourceType};

fn main() {
    let mut source = read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/debug_input.ts"
    ))
    .unwrap();

    let allocator = Allocator::default();
    let ret = transpile(&allocator, SourceType::ts(), &mut source);
    if ret.parser_panicked {
        dbg!(ret.parser_errors);
        abort();
    }
    println!("{}", source);
}
