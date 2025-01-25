use std::{env::args, fs::read_to_string, hint::black_box};

use oxidase_bench::{Benchee, OxcParser, Oxidase, SwcFastTsStrip};

fn main() {
    let benchee = args().nth(1).expect("benchee name");
    let input_path = args().nth(2).expect("input");
    let mut source = read_to_string(input_path).unwrap();
    match benchee.as_str() {
        "oxidase" => {
            black_box(Oxidase::default().run(&mut source));
        }
        "oxc_parser" => {
            black_box(OxcParser::default().run(&mut source));
        }
        "swc_fast_ts_strip" => {
            black_box(SwcFastTsStrip::default().run(&mut source));
        }
        other => panic!("Unrecogized benchee name: {}", other),
    };
}
