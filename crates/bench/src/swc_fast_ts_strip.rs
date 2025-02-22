use std::mem::take;

use swc_common::{errors::HANDLER, source_map::SourceMap, sync::Lrc, GLOBALS};
use swc_ecma_parser::TsSyntax;
use swc_error_reporters::handler::try_with_handler;
use swc_fast_ts_strip::{operate, Mode, Options};

use crate::Benchee;

#[derive(Default)]
pub struct SwcFastTsStrip(());

impl Benchee for SwcFastTsStrip {
    const NAME: &str = "swc_fast_ts_strip";
    fn run(&mut self, source: &mut String) {
        let cm = Lrc::new(SourceMap::new(swc_common::FilePathMapping::empty()));
        let input = take(source);

        let output = GLOBALS.set(&Default::default(), || {
            try_with_handler(cm.clone(), Default::default(), |handler| {
                HANDLER.set(handler, || {
                    let output = operate(
                        &cm,
                        handler,
                        input,
                        Options {
                            module: None,
                            filename: None,
                            parser: TsSyntax {
                                decorators: true,
                                ..Default::default()
                            },
                            mode: Mode::StripOnly,
                            transform: None,
                            ..Default::default()
                        },
                    )
                    .unwrap();

                    Ok(output.code)
                })
            })
            .unwrap()
        });
        *source = output;
    }
}
