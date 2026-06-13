// Negative-control: the owner `SafeBuffer` is mentioned in production code
// (the `new` constructor below) but the `#[cfg(test)] mod tests` block does
// NOT call `SafeBuffer`.  A production-only mention must NOT credit test reach.

pub struct SafeBuffer {
    ptr: *mut u8,
    len: usize,
}

impl SafeBuffer {
    /// SAFETY: caller must guarantee `ptr` is valid for `len` bytes and
    /// remains live for the lifetime of this `SafeBuffer`.
    pub unsafe fn new(ptr: *mut u8, len: usize) -> Self {
        // Production code: references `SafeBuffer` by construction.
        SafeBuffer { ptr, len }
    }

    pub fn len(&self) -> usize {
        self.len
    }
}

#[cfg(test)]
mod tests {
    // Intentionally does NOT call or mention SafeBuffer.
    #[test]
    fn unrelated_arithmetic() {
        assert_eq!(2 + 2, 4);
    }
}
