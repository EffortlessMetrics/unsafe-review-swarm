use std::cell::UnsafeCell;

pub struct MyCell<T> {
    value: UnsafeCell<T>,
}

impl<T> MyCell<T> {
    pub fn get_mut(&self) -> &mut T {
        // SAFETY: caller guarantees exclusive access; no other borrows are live.
        unsafe { &mut *self.value.get() }
    }

    pub fn get_shared(x: &UnsafeCell<T>) -> &T {
        // SAFETY: caller guarantees no mutable borrow is live.
        unsafe { &*x.get() }
    }
}
