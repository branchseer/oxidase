#![recursion_limit = "256"]

use std::env::args;
use std::process;
use std::fs::read_to_string;

use oxidase::oxc_diagnostics::NamedSource;
use oxidase::{transpile, Allocator, SourceType};

fn main() {
    let path = args().nth(1).unwrap_or_else(|| concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/bin/debug_input.ts"
    ).to_owned());

    let mut source = read_to_string(&path).unwrap();

    let allocator = Allocator::default();
    let ret = transpile(&allocator, SourceType::ts(), &mut source);
    if ret.parser_panicked {
        for err in ret.parser_errors {
            let named_source = NamedSource::new(&path, source.clone());
            eprintln!("{:?}", err.with_source_code(named_source));
        }
        process::exit(1);
    }
    println!("{}", source);
}
