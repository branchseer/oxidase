use std::ops::Range;

use oxc_allocator::{Allocator, String};
use oxc_span::Span;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patch<'a> {
    pub span: Span,
    pub replacement: &'a str,
    // origin_span: Option<Span>,
}

struct BackwardCursor<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl<'a> BackwardCursor<'a> {
    pub fn buf(&self) -> &[u8] {
        &self.buf
    }
    pub fn new(buf: &'a mut [u8]) -> Self {
        Self { pos: buf.len(), buf }
    }
    /// moves cursor back by `len`, and returns the slice passed by
    pub fn back_by(&mut self, len: usize) -> & mut [u8] {
        self.pos -= len;
        &mut self.buf[self.pos..(self.pos + len)]
    }
    pub fn write(&mut self, data: &[u8]) {
        self.back_by(data.len()).copy_from_slice(data);
    }
    pub fn write_byte(&mut self, data: u8) {
        self.pos -= 1;
        self.buf[self.pos] = data;
    }

    pub fn write_within(&mut self, src: Range<usize>) {
        self.pos -= src.len();
        self.buf.copy_within(src, self.pos);
    }
}

fn fill_with_whitespace_preserving_newlines(buf: &mut [u8]) {
    let mut i = 0;
    while i < buf.len() {
        // https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#table-line-terminator-code-points
        if matches!(buf[i], b'\r' | b'\n') {
            i += 1;
            continue;
        }

        const LS: [u8; 3] = [226, 128, 168];
        const PS: [u8; 3] = [226, 128, 169];
        if buf[i..].starts_with(&LS) || buf[i..].starts_with(&PS) {
            i += 3;
            continue;
        }
        buf[i] = b' ';
        i += 1;
    }
}

/// Panics if a span of any patch is not char boundary.
pub fn apply_patches<'alloc>(
    allocator: &'alloc Allocator,
    patches: &mut [Patch<'alloc>],
    source: &mut String<'_>,
) {
    if patches.is_empty() {
        return;
    }
    if cfg!(debug_assertions) {
        for i in 0..patches.len() - 1 {
            if patches[i].span.end > patches[i + 1].span.start {
                panic!("Unordered/overlapped patches: {:?}", patches)
            }
        }
        for patch in &*patches {
            if patch.span.end < patch.span.start {
                panic!("Invalid patch span: {:?}", patch);
            }
            if patch.replacement.contains('\n') {
                panic!("Patch replacement contains newlines: {:?}", patch);
            }
        }
    }

    let mut is_any_replacement_exceeded = false;
    let source_str = source.as_str();
    let mut patched_source_len = source_str.len();

    // The moving patch is the first patch whose replacement is larger than its span,
    // From this patch on, substrings between patches need to be moved.
    let mut moving_patch_start = patches.len();
    let mut size_to_add: usize = 0;

    for (i, patch) in patches.iter().enumerate() {
        let span_size = patch.span.size() as usize;
        if patch.replacement.len() > span_size && moving_patch_start == patches.len() {
            moving_patch_start = i;
        }

        size_to_add += patch.replacement.len().checked_sub(span_size).unwrap_or(0);
    }

    let source_bytes = unsafe { source.as_mut_vec() };
    source_bytes.resize(source_bytes.len() + size_to_add, 0);

    let mut cur = BackwardCursor::new(source_bytes.as_mut_slice());
    let mut last_patch_start: usize = cur.buf().len();

    for i in (moving_patch_start..patches.len()).rev() {
        let patch = &patches[i];

        let patch_start = patch.span.start as usize;
        let patch_end = patch.span.end as usize;

        // move the substring after the patch replacement
        cur.write_within(patch_end..last_patch_start);

        // insert the replacement
        let origin_len = patch_end - patch_start;
        let whitespaces_after_replacement_len = origin_len.checked_sub(patch.replacement.len()).unwrap_or(0);
        fill_with_whitespace_preserving_newlines(cur.back_by(whitespaces_after_replacement_len));
        cur.write(patch.replacement.as_bytes());

        last_patch_start = patch_start;
    }
    for i in (0..moving_patch_start).rev() {
        let patch = &patches[i];
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;
//     #[test]
//     fn blank_space() {
//         let allocator = Allocator::default();
//         let mut source = Source::Borrowed("abcd");
//         let mut patches = [Patch {
//             span: (1..3).into(),
//             replacement: "0",
//         }];
//         apply_patches(&allocator, &mut patches, true, &mut source);
//         assert_eq!(source.as_str(), "a0 d");
//     }
//     #[test]
//     fn blank_space_disable() {
//         let allocator = Allocator::default();
//         let mut source = Source::Borrowed("abcd");
//         let mut patches = [Patch {
//             span: (1..3).into(),
//             replacement: "0",
//         }];
//         apply_patches(&allocator, &mut patches, false, &mut source);
//         assert_eq!(source.as_str(), "a0d");
//     }
//     #[test]
//     fn exceeded() {
//         let allocator = Allocator::default();
//         let mut source = Source::Borrowed("abcd");
//         let mut patches = [Patch {
//             span: (1..3).into(),
//             replacement: "1234",
//         }];
//         apply_patches(&allocator, &mut patches, false, &mut source);
//         assert_eq!(source.as_str(), "a1234d");
//     }
// }
