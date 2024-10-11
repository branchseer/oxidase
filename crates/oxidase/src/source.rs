// use core::str;

use core::{slice, str};
use std::ptr::NonNull;

pub enum Source<'a> {
    Borrowed(&'a str),
    MutBorrowed(&'a mut str),
    Owned(String), // TODO: make it generic: BorrowMut<str> + Extend<str> + From<&str>
}

impl<'a> Source<'a> {
    pub fn to_mut(&mut self) -> &mut str {
        match self {
            Source::Borrowed(borrowed) => {
                *self = Self::Owned(borrowed.to_owned());
                match self {
                    Self::Owned(owned) => owned.as_mut_str(),
                    _ => unreachable!(),
                }
            }
            Source::MutBorrowed(mut_ref) => *mut_ref,
            Source::Owned(owned) => owned.as_mut_str(),
        }
    }
    /// the param `&mut &mut [u8]` is guaranteed to point to valid utf8 when passed to f.
    /// # Safety
    /// - The len returned by f <= len of the param `&mut [u8]`
    /// - if f returns, param[..len] must be valid utf8
    /// - if f panics, param must be valid utf8
    pub unsafe fn with_mut<R, F: FnOnce(&mut [u8]) -> (usize, R)>(&mut self, f: F) -> R {
        self.to_mut();
        match self {
            Source::Borrowed(_) => unreachable!(),
            Source::MutBorrowed(mut_ref) => {
                let mut bytes = str::as_bytes_mut(*mut_ref);

                let (len, ret) = f(&mut bytes);

                // At this point bytes is droped, and mut_ref might point to invalid utf8,
                // but non-UTF-8 str doesn't cause immediate UB: https://github.com/rust-lang/reference/pull/792,
                // and library-level non-UTF-8 UB is not triggered because we don't use *mut_ref

                // Before constructing a shortened &mut str, reset *mut_ref so that the original &mut str doesn't exist.
                // Otherwise the shortened &mut str is aliasing the original &mut str.
                *mut_ref = Box::leak(Box::<str>::default());
                *mut_ref = str::from_utf8_unchecked_mut(slice::from_raw_parts_mut(
                    bytes.as_mut_ptr(),
                    len,
                ));
                ret
            }
            Source::Owned(owned) => {
                let bytes_vec = owned.as_mut_vec();

                let mut bytes = bytes_vec.as_mut_slice();
                let (len, ret) = f(&mut bytes);

                bytes_vec.set_len(len);

                ret
            }
        }
    }
    pub fn to_owned(&mut self) -> &mut String {
        let borrowed = match self {
            Source::Borrowed(borrowed) => *borrowed,
            Source::MutBorrowed(mut_ref) => *mut_ref,
            Source::Owned(owned) => return owned,
        };
        *self = Self::Owned(borrowed.to_owned());
        match self {
            Self::Owned(owned) => owned,
            _ => unreachable!(),
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            Source::Borrowed(borrowed) => *borrowed,
            Source::MutBorrowed(mut_borrowed) => *mut_borrowed,
            Source::Owned(owned) => owned.as_str(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use rstest_reuse::{self, *};

    fn with_source_borrowed(s: &str, f: fn(Source)) {
        f(Source::Borrowed(s))
    }
    fn with_source_mut_borrowed(s: &str, f: fn(Source)) {
        let mut s = s.to_owned();
        f(Source::MutBorrowed(s.as_mut_str()))
    }
    fn with_source_owned(s: &str, f: fn(Source)) {
        let s: String = s.to_owned();
        f(Source::Owned(s))
    }

    type WithSourceFn = fn(&str, fn(Source));

    #[template]
    #[rstest]
    #[case(with_source_borrowed)]
    #[case(with_source_mut_borrowed)]
    #[case(with_source_owned)]
    // Define a and b as cases arguments
    fn with_source_cases(#[case] with_source: WithSourceFn) {}

    #[apply(with_source_cases)]
    fn source_to_mut(with_source: WithSourceFn) {
        with_source("hello", |mut source| {
            assert_eq!(source.to_mut(), "hello");
        })
    }

    #[apply(with_source_cases)]
    fn source_with_mut(with_source: WithSourceFn) {
        with_source("你好", |mut source| {
            unsafe {
                source.with_mut(|buf| {
                    buf[0] = b'a';
                    (1, ())
                });
            }
            assert_eq!(source.as_str(), "a");
        })
    }
}
