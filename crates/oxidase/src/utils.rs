use oxc_allocator::{Allocator, Vec};

/// Get the index of `elem` reference in `slice`.
pub fn index_of<T>(elem: &T, slice: &[T]) -> usize {
    (elem as *const T as usize - slice.as_ptr() as usize) / core::mem::size_of::<T>()
}

pub fn insert_after_last<'alloc, T>(
    v: &mut Vec<'alloc, T>,
    mut predicate: impl FnMut(&T) -> bool,
    val: T,
) {
    let last_matched_index =
        v.iter()
            .enumerate()
            .rev()
            .find_map(|(index, elem)| if predicate(elem) { Some(index) } else { None });
    let insert_pos = if let Some(last_matched_index) = last_matched_index {
        last_matched_index + 1
    } else {
        0
    };
    v.insert(insert_pos, val);
}

#[derive(Debug)]
pub struct Stack1<'a, T> {
    first: T,
    rest: Vec<'a, T>,
}
impl<'a, T> Stack1<'a, T> {
    pub fn new(first: T, rest_capacity: usize, allocator: &'a Allocator) -> Self {
        Self {
            first,
            rest: Vec::with_capacity_in(rest_capacity, allocator),
        }
    }
    pub fn last(&self) -> &T {
        self.rest.last().unwrap_or(&self.first)
    }
    pub fn last_mut(&mut self) -> &mut T {
        self.rest.last_mut().unwrap_or(&mut self.first)
    }
    pub fn pop(&mut self) -> Option<T> {
        self.rest.pop()
    }
    pub fn push(&mut self, value: T) {
        self.rest.push(value)
    }
}

#[cfg(test)]
mod tests {
    use oxc_allocator::Allocator;

    use super::*;

    #[test]
    fn index_of_basic() {
        let nums = &[1, 2, 3];
        let elem = &nums[1];
        assert_eq!(index_of(elem, nums), 1);
    }

    #[test]
    fn insert_after_last_basic() {
        let allocator = Allocator::default();
        let mut v = oxc_allocator::Vec::from_iter_in([1, 2, 4], &allocator);
        insert_after_last(&mut v, |e| *e <= 2, 3);
        assert_eq!(v.as_slice(), [1, 2, 3, 4].as_slice());
    }
    #[test]
    fn insert_after_last_empty() {
        let allocator = Allocator::default();
        let mut v = oxc_allocator::Vec::from_iter_in([], &allocator);
        insert_after_last(&mut v, |e| *e <= 2, 3);
        assert_eq!(v.as_slice(), [3].as_slice());
    }
    #[test]
    fn insert_after_last_all_matched() {
        let allocator = Allocator::default();
        let mut v = oxc_allocator::Vec::from_iter_in([1, 2, 4], &allocator);
        insert_after_last(&mut v, |e| *e >= 0, 3);
        assert_eq!(v.as_slice(), [1, 2, 4, 3].as_slice());
    }
    #[test]
    fn insert_after_last_none_matched() {
        let allocator = Allocator::default();
        let mut v = oxc_allocator::Vec::from_iter_in([1, 2, 4], &allocator);
        insert_after_last(&mut v, |e| *e < 0, 3);
        assert_eq!(v.as_slice(), [3, 1, 2, 4].as_slice());
    }
}
