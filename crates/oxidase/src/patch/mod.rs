use std::{
    mem::{transmute, MaybeUninit},
    ops::Range,
    slice::from_raw_parts_mut,
};

use oxc_span::Span;

use crate::string_buf::StringBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Patch<'a> {
    pub span: Span,
    pub replacement: &'a str,
}

impl<'a> From<Span> for Patch<'a> {
    fn from(span: Span) -> Self {
        Patch {
            span,
            replacement: "",
        }
    }
}

impl<'a> From<Range<u32>> for Patch<'a> {
    fn from(range: Range<u32>) -> Self {
        Patch {
            span: range.into(),
            replacement: "",
        }
    }
}

impl<'a, S: Into<Span>> From<(S, &'a str)> for Patch<'a> {
    fn from((span, replacement): (S, &'a str)) -> Self {
        Patch {
            span: span.into(),
            replacement,
        }
    }
}

struct BackwardCursor<'a> {
    buf: &'a mut [MaybeUninit<u8>],
    pos: usize,
}

impl<'a> BackwardCursor<'a> {
    // pub fn buf(&self) -> &[u8] {
    //     &self.buf
    // }
    pub fn new(buf: &'a mut [MaybeUninit<u8>]) -> Self {
        Self {
            pos: buf.len(),
            buf,
        }
    }
    /// moves cursor back by `len`, and returns the slice passed by
    fn back_by(&mut self, len: usize) -> &mut [MaybeUninit<u8>] {
        self.pos -= len;
        &mut self.buf[self.pos..(self.pos + len)]
    }
    #[inline]
    pub fn write(&mut self, src: &[u8]) {
        // SAFETY: &[u8] and &[MaybeUninit<u8>] have the same layout
        let uninit_src: &[MaybeUninit<u8>] = unsafe { transmute(src) };
        self.back_by(uninit_src.len()).copy_from_slice(uninit_src);
    }
    #[inline]
    pub fn write_byte(&mut self, data: u8) {
        self.pos -= 1;
        self.buf[self.pos] = MaybeUninit::new(data);
    }

    #[inline]
    pub fn write_within(&mut self, src: Range<usize>) {
        let dest_start = self.pos - src.len();
        if src.start != dest_start {
            self.buf.copy_within(src, dest_start);
        }
        self.pos = dest_start;
    }

    /// Safety: self.buf[..src.end] must be inititialized.
    #[inline]
    pub unsafe fn write_whitespaces_preserving_newlines(&mut self, src: Range<usize>) {
        // let mut src_index = src.end as isize - 1;
        // let Some(mut src_index) = src.end.checked_sub(1) else {
        //     return;
        // };

        let mut src_index = src.end.checked_sub(1);
        while let Some(unwrapped_src_index) = src_index {
            if unwrapped_src_index < src.start {
                break;
            }
            let byte = self.buf.get_unchecked(unwrapped_src_index).assume_init();
            match byte {
                b'\r' | b'\n' => {
                    self.write_byte(byte);
                    src_index = unwrapped_src_index.checked_sub(1);
                }
                168 | 169
                    if matches!(
                        transmute::<&[MaybeUninit<u8>], &[u8]>(
                            &self.buf.get_unchecked(..unwrapped_src_index)
                        ),
                        [.., 226, 128]
                    ) =>
                {
                    self.write(&[226, 128, byte]);
                    src_index = unwrapped_src_index.checked_sub(3);
                }
                _ => {
                    self.write_byte(b' ');
                    src_index = unwrapped_src_index.checked_sub(1);
                }
            }
        }
    }
}

/// # Safety
///
/// - patches are sorted and not overlapped
/// - patche spans are valid utf8 char boundaries
///
///  Panics if a span of any patch is not char boundary.
pub unsafe fn apply_patches(patches: &[Patch<'_>], source: &mut impl StringBuf) {
    if patches.is_empty() {
        return;
    }
    if cfg!(debug_assertions) {
        for i in 0..patches.len() - 1 {
            if patches[i].span.end > patches[i + 1].span.start {
                panic!("Unordered/overlapped patches: {:?}", patches)
            }
        }
        for patch in patches {
            if patch.span.end < patch.span.start {
                panic!("Invalid patch span: {:?}", patch);
            }
            if patch.replacement.contains('\n') {
                panic!("Patch replacement contains newlines: {:?}", patch);
            }
        }
    }

    let src_len = source.as_ref().len();
    let mut additional: usize = 0;

    for patch in patches.iter() {
        let span_size = patch.span.size() as usize;
        additional += patch.replacement.len().saturating_sub(span_size);
    }

    let mut last_patch_start: usize = src_len;

    source.reserve(additional);

    let mut cur = BackwardCursor::new(unsafe {
        from_raw_parts_mut(
            source.as_mut_ptr() as *mut MaybeUninit<u8>,
            src_len + additional,
        )
    });

    for patch in patches.iter().rev() {
        let patch_start = patch.span.start as usize;
        let patch_end = patch.span.end as usize;

        // write substring after patch span
        cur.write_within(patch_end..last_patch_start);

        // write whitespaces after replacement
        unsafe {
            cur.write_whitespaces_preserving_newlines(
                (patch_start + patch.replacement.len())..patch_end,
            )
        };

        #[cfg(debug_assertions)]
        {
            use crate::line_term::contains_line_terminators;
            use std::cmp::min;
            use std::str::from_utf8;

            let source_to_be_replaced = unsafe {
                transmute::<&[MaybeUninit<u8>], &[u8]>(
                    &cur.buf[patch_start..min(patch_start + patch.replacement.len(), patch_end)],
                )
            };
            assert!(
                !contains_line_terminators(source_to_be_replaced),
                "Source to be replaced (replacement is {:?}) should not contain line terminators: {:?}",
                patch.replacement, from_utf8(source_to_be_replaced).unwrap()
            );
            assert!(
                !contains_line_terminators(patch.replacement.as_bytes()),
                "Replacement (source to be replaced is {:?}) should not contain line terminators: {:?}",
                from_utf8(source_to_be_replaced).unwrap(), patch.replacement
            );
        }

        // write replacement
        cur.write(patch.replacement.as_bytes());

        last_patch_start = patch_start;
    }
    cur.write_within(0..last_patch_start);

    debug_assert_eq!(cur.pos, 0);
    debug_assert!(core::str::from_utf8(unsafe {
        transmute::<&mut [std::mem::MaybeUninit<u8>], &[u8]>(cur.buf)
    })
    .is_ok());

    unsafe { source.set_len(src_len + additional) };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic() {
        let mut source = "abc\nd".to_owned();
        let patches = [
            Patch {
                span: (0..0).into(),
                replacement: "x",
            },
            Patch {
                span: (1..3).into(),
                replacement: "0",
            },
        ];
        unsafe { apply_patches(&patches, &mut source) };
        assert_eq!(source.as_str(), "xa0 \nd");
    }

    #[test]
    fn all_removed() {
        let mut source = "abc\nd".to_owned();
        let patches = [Patch {
            span: (0..source.len() as u32).into(),
            replacement: "",
        }];
        unsafe { apply_patches(&patches, &mut source) };
        assert_eq!(source.as_str(), "   \n ");
    }
}
