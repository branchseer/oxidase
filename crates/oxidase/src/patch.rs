use oxc_allocator::{Allocator, String, Vec};
use oxc_span::Span;

use crate::source::Source;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Patch<'a> {
    pub span: Span,
    pub replacement: &'a str,
    // origin_span: Option<Span>,
}

/// Panics if a span of any patch is not char boundary.
pub fn apply_patches<'alloc>(
    allocator: &'alloc Allocator,
    patches: &mut [Patch<'alloc>],
    prefer_blank_space: bool,
    source: &mut Source<'_, 'alloc>,
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
    }
    let mut is_any_replacement_exceeded = false;
    let source_str = source.as_str();
    let mut patched_source_len = source_str.len();
    for patch in patches.iter() {
        let span_size = patch.span.size() as usize;
        is_any_replacement_exceeded =
            is_any_replacement_exceeded || patch.replacement.len() > span_size;
        patched_source_len -= span_size;
        patched_source_len += patch.replacement.len();

        assert!(source_str.is_char_boundary(patch.span.start as usize));
        assert!(source_str.is_char_boundary(patch.span.start as usize));
    }

    if !is_any_replacement_exceeded {
        if prefer_blank_space {
            let source = source.to_mut(allocator);
            for patch in patches {
                unsafe {
                    let source_to_replace = &mut source.as_bytes_mut()
                        [patch.span.start as usize..patch.span.end as usize];
                    source_to_replace[..patch.replacement.len()]
                        .copy_from_slice(patch.replacement.as_bytes());
                    source_to_replace[patch.replacement.len()..].fill(b' ');
                }
            }
        } else {
            // sort is faster than sort_unstable when the slice is partially sorted.
            // patches.sort_by_key(|patch| patch.span.start);
            // ！is_any_replacement_exceeded && !prefer_blank_space
            // Copy patch replacements and substring between patches from left to right. No new string alloc needed
            // Safety:
            //
            unsafe {
                let bytes = source.to_owned(allocator).as_mut_vec();
                let mut cur_pos = patches[0].span.start as usize;
                for (i, patch) in patches.iter().enumerate() {
                    let end = patch.span.end as usize;

                    // Append replacement
                    bytes[cur_pos..(cur_pos + patch.replacement.len())]
                        .copy_from_slice(patch.replacement.as_bytes());

                    cur_pos += patch.replacement.len();

                    // Append content between current and next patch (or the end)
                    let next_patch_start = if let Some(next_patch) = patches.get(i + 1) {
                        next_patch.span.start as usize
                    } else {
                        bytes.len()
                    };
                    bytes.copy_within(end..next_patch_start, cur_pos);
                    cur_pos += next_patch_start - end;
                }
                bytes.set_len(cur_pos);
            }
        }
    } else {
        // sort is faster than sort_unstable when the slice is partially sorted.
        // patches.sort_by_key(|patch| patch.span.start);
    
        // is_any_replacement_exceeded
        // Replacement might overrides substrings between patches. Allocating new string
        let mut out = String::with_capacity_in(patched_source_len, allocator);

        let mut start = 0usize;
        for patch in patches {
            // Safety: patch span boundaries are validated at the beginning of this function
            out.push_str(unsafe { source_str.get_unchecked(start..patch.span.start as usize) });
            out.push_str(patch.replacement);
            start = patch.span.end as usize;
        }
        // Safety: patch span boundaries are validated at the beginning of this function
        out.push_str(unsafe { source_str.get_unchecked(start..) });

        *source = Source::Owned(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn blank_space() {
        let allocator = Allocator::default();
        let mut source = Source::Borrowed("abcd");
        let mut patches = [Patch {
            span: (1..3).into(),
            replacement: "0",
        }];
        apply_patches(&allocator, &mut patches, true, &mut source);
        assert_eq!(source.as_str(), "a0 d");
    }
    #[test]
    fn blank_space_disable() {
        let allocator = Allocator::default();
        let mut source = Source::Borrowed("abcd");
        let mut patches = [Patch {
            span: (1..3).into(),
            replacement: "0",
        }];
        apply_patches(&allocator, &mut patches, false, &mut source);
        assert_eq!(source.as_str(), "a0d");
    }
    #[test]
    fn exceeded() {
        let allocator = Allocator::default();
        let mut source = Source::Borrowed("abcd");
        let mut patches = [Patch {
            span: (1..3).into(),
            replacement: "1234",
        }];
        apply_patches(&allocator, &mut patches, false, &mut source);
        assert_eq!(source.as_str(), "a1234d");
    }
}
