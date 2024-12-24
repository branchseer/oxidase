use std::ops::Range;

use oxc_allocator::String;
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
    // pub fn buf(&self) -> &[u8] {
    //     &self.buf
    // }
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
        let dest_start = self.pos - src.len();
        if src.start != dest_start {
            self.buf.copy_within(src, dest_start);
        }
        self.pos = dest_start;
    }

    pub fn write_whitespaces_preserving_newlines(&mut self, src: Range<usize>) {
        let mut src_index = src.end as isize - 1;
        'scan_src: while src_index >= src.start as isize {
            // https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#table-line-terminator-code-points
            const LS: &[u8] = &[226, 128, 168];
            const PS: &[u8] = &[226, 128, 169];

            for line_terminator in [b"\n", b"\r", LS, PS] {
                if self.buf[..=src_index as usize].ends_with(line_terminator) {
                    self.write(line_terminator);
                    src_index -= line_terminator.len() as isize;
                    continue 'scan_src;
                }
            }
            self.write_byte(b' ');
            src_index -= 1;
        }
    }
}

/// Panics if a span of any patch is not char boundary.
pub fn apply_patches<'alloc>(
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

    let mut patched_source_len = source.len();

    for patch in patches.iter() {
        let span_size = patch.span.size() as usize;
        patched_source_len += patch.replacement.len().checked_sub(span_size).unwrap_or(0);
    }

    let mut last_patch_start: usize = source.len();

    let source_bytes = unsafe { source.as_mut_vec() };
    source_bytes.resize(patched_source_len, 0);

    let mut cur = BackwardCursor::new(source_bytes.as_mut_slice());

    for patch in patches.iter().rev() {

        let patch_start = patch.span.start as usize;
        let patch_end = patch.span.end as usize;

        // write substring after patch span
        cur.write_within(patch_end..last_patch_start);

        // write whitespaces after replacement
        cur.write_whitespaces_preserving_newlines((patch_start + patch.replacement.len())..patch_end);

        // write replacement
        cur.write(patch.replacement.as_bytes());
        
        last_patch_start = patch_start;
    }
    cur.write_within(0..last_patch_start);

    debug_assert_eq!(cur.pos, 0);
    debug_assert!(core::str::from_utf8(source_bytes.as_slice()).is_ok());
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;

    use super::*;
    #[test]
    fn basic() {
        let allocator = Allocator::default();
        let mut source = String::from_str_in("abc\nd", &allocator);
        let mut patches = [Patch {
            span: (0..0).into(),
            replacement: "x",
        }, Patch {
            span: (1..3).into(),
            replacement: "0",
        }];
        apply_patches(&mut patches,  &mut source);
        assert_eq!(source.as_str(), "xa0 \nd");
    }

    #[test]
    fn all_removed() {
        let allocator = Allocator::default();
        let mut source = String::from_str_in("abc\nd", &allocator);
        let mut patches = [Patch {
            span: (0..source.len() as u32).into(),
            replacement: "",
        }];
        apply_patches(&mut patches,  &mut source);
        assert_eq!(source.as_str(), "   \n ");
    }
}
