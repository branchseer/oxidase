use oxc_allocator::Allocator;
use oxc_allocator::String;

pub enum Source<'borrowed, 'alloc> {
    Borrowed(&'borrowed str),
    Owned(String<'alloc>),
}

impl<'borrowed, 'alloc> Source<'borrowed, 'alloc> {
    pub fn to_mut(&mut self, allocator: &'alloc Allocator) -> &mut str {
        match self {
            Source::Borrowed(borrowed) => {
                *self = Self::Owned(String::from_str_in(*borrowed, &allocator));
                match self {
                    Self::Owned(owned) => owned.as_mut_str(),
                    _ => unreachable!(),
                }
            }
            Source::Owned(owned) => owned.as_mut_str(),
        }
    }
    pub fn to_owned(&mut self, allocator: &'alloc Allocator) -> &mut String<'alloc> {
        let borrowed = match self {
            Source::Borrowed(borrowed) => *borrowed,
            Source::Owned(owned) => return owned,
        };
        *self = Self::Owned(String::from_str_in(borrowed, &allocator));
        match self {
            Self::Owned(owned) => owned,
            _ => unreachable!(),
        }
    }
    pub fn as_str(&self) -> &str {
        match self {
            Source::Borrowed(borrowed) => *borrowed,
            Source::Owned(owned) => owned.as_str(),
        }
    }
}
