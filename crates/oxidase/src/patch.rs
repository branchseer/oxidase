use copy_from_str::CopyFromStrExt;
use oxc_allocator::Vec;
use oxc_span::Span;

use crate::source::Source;

#[derive(Debug)]
pub struct Patch<'a> {
    pub span: Span,
    pub replacement: &'a str,
    // origin_span: Option<Span>,
}

/// Panics if a span of any patch is not char boundary.
pub fn apply<'alloc, 'source>(
    patches: &[Patch<'alloc>],
    prefer_blank_space: bool,
    source: &mut Source<'source>,
) {
    if patches.is_empty() {
        return;
    }

    if patches
        .iter()
        .all(|patch| patch.replacement.len() <= patch.span.size() as usize)
    {
        if prefer_blank_space {
            let source = source.to_mut();
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
            unsafe {
                source.with_mut(|bytes| {
                    let mut cur_pos = patches[0].span.start as usize;
                    for (i, patch) in patches.iter().enumerate() {
                        let end = patch.span.end as usize;
                        /*
                        ******|---    |*****|---    |****
                               ^patch        ^next_patch
                        */

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

                    (0, ())
                })
            }
        }
    }
    todo!()
}
