pub fn as_mut<T>(p: *const T) -> &'static mut T {
    // SAFETY: caller provides a valid, exclusively-owned, long-lived pointer.
    unsafe { &mut *(p as *mut T) }
}
