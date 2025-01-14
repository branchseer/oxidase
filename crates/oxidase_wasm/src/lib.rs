mod utils;

use std::cell::RefCell;

use wasm_bindgen::prelude::*;

thread_local! {
    static ALLOCATOR: RefCell<oxidase::Allocator> = Default::default();
}

#[wasm_bindgen]
pub fn transpile(mut source: String) -> String {
    console_error_panic_hook::set_once();
    ALLOCATOR.with_borrow_mut(|allocator| {
        let ret = oxidase::transpile(allocator, oxidase::SourceType::ts(), &mut source);  
        allocator.reset();
        source
    })
}
