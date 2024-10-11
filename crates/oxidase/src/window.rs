use core::borrow::Borrow;
use core::convert::AsRef;
use core::ops::Deref;

#[derive(Debug, Clone, Copy)]
pub struct Window<T, const CAP: usize>([T; CAP]);

impl<T: Copy, const CAP: usize> Window<T, CAP> {
    pub fn repeating(val: T) -> Self {
        Self([val; CAP])
    }
}

impl<T: Default + Copy, const CAP: usize> Default for Window<T, CAP> {
    fn default() -> Self {
        Self([T::default(); CAP])
    }
}

impl<T: Copy, const CAP: usize> Window<T, CAP> {
    pub fn push(&mut self, value: T) {
        self.0.copy_within(1.., 0);
        self.0[CAP - 1] = value;
    }
}
impl<T, const CAP: usize> AsRef<[T; CAP]> for Window<T, CAP> {
    fn as_ref(&self) -> &[T; CAP] {
        &self.0
    }
}

impl<T, const CAP: usize> Borrow<[T; CAP]> for Window<T, CAP> {
    fn borrow(&self) -> &[T; CAP] {
        &self.0
    }
}

impl<T, const CAP: usize> Deref for Window<T, CAP> {
    type Target = [T; CAP];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T, const CAP: usize> Window<T, CAP> {
    fn as_slice(&self) -> &[T; CAP] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::Window;

    #[test]
    fn test_window_push() {
        let mut deque = Window::<u32, 3>::default();
        assert_eq!(&*deque, &[0, 0, 0]);
        deque.push(1);
        assert_eq!(&*deque, &[0, 0, 1]);
        deque.push(2);
        assert_eq!(&*deque, &[0, 1, 2]);
        deque.push(3);
        assert_eq!(&*deque, &[1, 2, 3]);
        deque.push(4);
        assert_eq!(&*deque, &[2, 3, 4]);
    }
}
