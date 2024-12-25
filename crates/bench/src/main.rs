use std::{
    fs::read_to_string,
    path::{Path, PathBuf},
    sync::Arc,
};

use oxidase::{Allocator, SourceType, String};
use oxidase_tsc::Tsc;
use swc::try_with_handler;

fn main() {
    let fixture_ecosystem_dir: PathBuf =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../e2e/fixture/ecosystem");
    let mut tsc = Tsc::new();

    let mut allocator = Allocator::default();
    let source =
        read_to_string(fixture_ecosystem_dir.join("a.txt")).unwrap();
    let source = tsc.process_ts(&source, true).unwrap().ts;
    dbg!(source.len());
    let mut cm: Arc<swc_common::SourceMap> = Default::default();

    swc_common::GLOBALS.set(&Default::default(), || {
        try_with_handler(cm.clone(), Default::default(), |handler| {
            let input = "let b: string = 1".to_owned();
            
            let output =  swc_fast_ts_strip::operate(
                &cm,
                handler,
                input,
                swc_fast_ts_strip::Options::default(),
            )
            .unwrap();
            assert!(output.map.is_none());
            println!("{}", output.code);
            Ok(())
        })
        .unwrap();
    });
}
