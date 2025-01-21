use std::{cell::RefCell, fmt::Write};

use oxidase::oxc_diagnostics::NamedSource;
use wasm_bindgen::prelude::*;

/// SAFETY: The runtime environment must be single-threaded WASM.
#[cfg(target_family = "wasm")]
#[global_allocator]
static GLOBAL_ALLOCATOR: talc::TalckWasm = unsafe { talc::TalckWasm::new_global() };

thread_local! {
    static ALLOCATOR: RefCell<oxidase::Allocator> = Default::default();
}

#[wasm_bindgen]
pub fn transpile(path: &str, mut source: String) -> Result<String, JsError> {
    console_error_panic_hook::set_once();
    ALLOCATOR.with_borrow_mut(|allocator| {
        let ret = oxidase::transpile(
            allocator,
            oxidase::SourceType::from_path(path)?,
            &mut source,
        );
        allocator.reset();
        if ret.parser_panicked {
            let mut error_msg = String::new();
            for error in ret.parser_errors {
                error_msg
                    .write_fmt(format_args!(
                        "{:?}",
                        error.with_source_code(NamedSource::new(path, source.clone()))
                    ))
                    .unwrap();
            }
            return Err(JsError::new(&error_msg));
        }
        Ok(source)
    })
}
