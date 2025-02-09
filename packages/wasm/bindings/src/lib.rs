use std::{cell::RefCell, fmt::Write};

use oxidase::{oxc_diagnostics::NamedSource, SourceType};
use wasm_bindgen::prelude::*;

/*
/// SAFETY: The runtime environment must be single-threaded WASM.
static GLOBAL_ALLOCATOR: talc::TalckWasm = unsafe { talc::TalckWasm::new_global() };
*/

thread_local! {
    static ALLOCATOR: RefCell<oxidase::Allocator> = Default::default();
}

#[wasm_bindgen]
pub fn transpile(mut source: String, path: Option<String>) -> Result<String, JsError> {
    console_error_panic_hook::set_once();
    let source_type = if let Some(path) = &path {
        SourceType::from_path(path)?
    } else {
        SourceType::ts()
    };
    ALLOCATOR.with_borrow_mut(|allocator| {
        let ret = oxidase::transpile(allocator, source_type, &mut source);
        allocator.reset();
        if ret.parser_panicked {
            let mut error_msg = String::new();
            for error in ret.parser_errors {
                let error = if let Some(path) = &path {
                    error.with_source_code(NamedSource::new(path, source.clone()))
                } else {
                    error.with_source_code(source.clone())
                };
                error_msg.write_fmt(format_args!("{:?}\n", error)).unwrap();
            }
            return Err(JsError::new(&error_msg));
        }
        Ok(source)
    })
}
