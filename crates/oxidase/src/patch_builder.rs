use std::ops::{Index, IndexMut};

use bumpalo::Bump;
use oxc_allocator::{Allocator, Vec};
use oxc_span::Span;

use crate::Patch;

pub struct PatchBuilder<'source, 'alloc> {
    source: &'source [u8],
    patches: Vec<'alloc, Patch<'alloc>>,
}

impl<'source, 'alloc> Index<usize> for PatchBuilder<'source, 'alloc> {
    type Output = Patch<'alloc>;

    fn index(&self, index: usize) -> &Self::Output {
        &self.patches[index]
    }
}
impl<'source, 'alloc> IndexMut<usize> for PatchBuilder<'source, 'alloc> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.patches.as_mut_slice()[index]
    }
}

impl<'source, 'alloc> PatchBuilder<'source, 'alloc> {
    pub fn new(source: &'source [u8], allocator: &'alloc Allocator) -> Self {
        Self {
            source,
            patches: Vec::new_in(allocator),
        }
    }
    pub fn len(&self) -> usize {
        self.patches.len()
    }
    pub fn push(&mut self, patch: impl Into<Patch<'alloc>>) {
        let patch = patch.into();
        if cfg!(debug_assertions) {
            if let Some(last_patch) = self.patches.last() {
                assert!(patch.span.start >= last_patch.span.end);
            }
        }
        self.patches.push(patch);
    }

    pub fn push_merging_tail(&mut self, patch: impl Into<Patch<'alloc>>) {
        let patch = patch.into();
        debug_assert!(
            patch.span.end >= self.patches.last().map(|patch| patch.span.end).unwrap_or(0)
        );

        while matches!(self.patches.last(), Some(last_patch) if last_patch.span.start >= patch.span.start)
        {
            self.patches.pop();
        }
        self.patches.push(patch);
    }

    pub fn binary_search_insert(&mut self, patch: impl Into<Patch<'alloc>>) {
        let patch = patch.into();
        let mut insert_pos = self.patches.len();
        while insert_pos > 0 && self.patches[insert_pos - 1].span.end > patch.span.start {
            insert_pos -= 1
        }
        if cfg!(debug_assertions) {
            if let Some(patch_after) = self.patches.get(insert_pos) {
                assert!(patch.span.end <= patch_after.span.start);
            }
        }
        self.patches.insert(insert_pos, patch);
    }
    pub fn patches(&self) -> &[Patch<'alloc>] {
        &self.patches
    }
    pub fn into_patches(self) -> Vec<'alloc, Patch<'alloc>> {
        self.patches
    }
    pub fn last(&self) -> Option<&Patch<'alloc>> {
        self.patches.last()
    }
    pub fn last_mut(&mut self) -> Option<&mut Patch<'alloc>> {
        self.patches.last_mut()
    }
    pub fn patches_mut(&mut self) -> &mut [Patch<'alloc>] {
        &mut self.patches
    }
    pub fn truncate(&mut self, len: usize) {
        self.patches.truncate(len);
    }
    pub fn insert(&mut self, index: usize, patch: impl Into<Patch<'alloc>>) {
        self.patches.insert(index, patch.into());
    }
}

