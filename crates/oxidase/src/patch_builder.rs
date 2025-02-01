use std::{
    cmp::min,
    ops::{Deref, DerefMut},
};

use oxc_allocator::{Allocator, Vec};

use crate::{line_term::contains_line_terminators, Patch};

pub struct PatchBuilder<'source, 'alloc> {
    source: &'source [u8],
    patches: Vec<'alloc, Patch<'alloc>>,
}

impl<'source, 'alloc> Deref for PatchBuilder<'source, 'alloc> {
    type Target = [Patch<'alloc>];

    fn deref(&self) -> &Self::Target {
        self.patches.as_slice()
    }
}
impl<'source, 'alloc> DerefMut for PatchBuilder<'source, 'alloc> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.patches.as_mut_slice()
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

    pub fn push_checking_line_terminator(&mut self, patch: impl Into<Patch<'alloc>>) {
        let patch = patch.into();
        let end = min(
            patch.span.end as usize,
            patch.span.start as usize + patch.replacement.len(),
        );
        let source_to_replace = &self.source[patch.span.start as usize..end];
        if contains_line_terminators(source_to_replace) {
            self.push((patch.span, ""));
            self.push(((patch.span.end..patch.span.end), patch.replacement));
        } else {
            self.push(patch);
        }
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
        let insert_pos = self
            .patches
            .partition_point(|p| p.span.end <= patch.span.start);

        if cfg!(debug_assertions) {
            if let Some(patch_after) = self.patches.get(insert_pos) {
                assert!(patch.span.end <= patch_after.span.start);
            }
        }
        self.patches.insert(insert_pos, patch);
    }

    // pub fn patches(&self) -> &[Patch<'alloc>] {
    //     &self.patches
    // }
    pub fn into_patches(self) -> Vec<'alloc, Patch<'alloc>> {
        self.patches
    }
    // pub fn last(&self) -> Option<&Patch<'alloc>> {
    //     self.patches.last()
    // }
    // pub fn last_mut(&mut self) -> Option<&mut Patch<'alloc>> {
    //     self.patches.last_mut()
    // }
    // pub fn patches_mut(&mut self) -> &mut [Patch<'alloc>] {
    //     &mut self.patches
    // }
    pub fn truncate(&mut self, len: usize) {
        self.patches.truncate(len);
    }
    pub fn insert(&mut self, index: usize, patch: impl Into<Patch<'alloc>>) {
        let patch = patch.into();
        if cfg!(debug_assertions) {
            if let Some(index_before) = index.checked_sub(1) {
                assert!(self.patches[index_before].span.end <= patch.span.start);
            }
            if let Some(patch_after) = self.patches.get(index + 1) {
                assert!(patch_after.span.start >= patch.span.end);
            }
        }
        self.patches.insert(index, patch);
    }
}
