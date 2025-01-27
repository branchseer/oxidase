use std::{alloc::{alloc, dealloc, Layout}, hint::black_box, ptr::write_volatile};

use wasm_bindgen::prelude::*;

use crate::{Benchee as _, OxcParser, Oxidase, SwcFastTsStrip};

fn page_count() -> usize {
    return core::arch::wasm32::memory_size(0);
}

#[wasm_bindgen]
pub enum Benchee {
    Oxidase, OxcParser, SwcFastTsStrip,
}

const PAGE_SIZE: usize = 65536;

#[wasm_bindgen]
pub fn measure_memory(benchee: Benchee, mut source: String) -> usize {
    let page_count_before = page_count();

    loop {
        const BYTE_LAYOUT: Layout = Layout::new::<u8>();
        let ptr = unsafe { alloc(BYTE_LAYOUT) };
        if ptr.is_null() {
            panic!("Failed to allocate")
        }
        unsafe { write_volatile(ptr, 1); } // prevent allocation from being optimized away
        if page_count() != page_count_before {
            unsafe { dealloc(ptr, BYTE_LAYOUT) };
            break;
        }
    }

    match benchee {
        Benchee::Oxidase => {
            black_box(Oxidase::default().run(&mut source));
        }
        Benchee::OxcParser =>  {
            black_box(OxcParser::default().run(&mut source));
        }
        Benchee::SwcFastTsStrip => {
            black_box(SwcFastTsStrip::default().run(&mut source));
        }
    };
    black_box(source);
    (page_count() - page_count_before) * PAGE_SIZE
}
