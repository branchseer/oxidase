pub trait StringBuf: AsRef<str> {
    fn as_mut_ptr(&mut self) -> *mut u8;
    fn reserve(&mut self, capacity: usize);
    unsafe fn set_len(&mut self, len: usize);
}

impl StringBuf for String {
    fn reserve(&mut self, additional: usize) {
        String::reserve(self, additional);
    }

    unsafe fn set_len(&mut self, new_len: usize) {
        self.as_mut_vec().set_len(new_len);
    }
    
    fn as_mut_ptr(&mut self) -> *mut u8 {
        unsafe { self.as_mut_vec().as_mut_ptr() }
    }
}
