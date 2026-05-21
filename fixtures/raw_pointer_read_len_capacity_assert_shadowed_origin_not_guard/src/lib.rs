pub struct ArrayVec<T, const CAP: usize> {
    len: usize,
    storage: [T; CAP],
}

impl<T, const CAP: usize> ArrayVec<T, CAP> {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn capacity(&self) -> usize {
        CAP
    }

    pub fn as_ptr(&self) -> *const T {
        self.storage.as_ptr()
    }
}

pub fn read_full_array<T: Copy, const CAP: usize>(
    values: &ArrayVec<T, CAP>,
    other: &ArrayVec<T, CAP>,
) -> [T; CAP] {
    debug_assert_eq!(values.len(), values.capacity());
    let values = other;
    let ptr = values.as_ptr();
    // SAFETY: fixture checks that len/capacity assertions over a shadowed origin are stale.
    unsafe { core::ptr::read(ptr as *const [T; CAP]) }
}

#[cfg(test)]
mod tests {
    use super::{ArrayVec, read_full_array};

    #[test]
    fn reaches_read_full_array() {
        let values = ArrayVec {
            len: 2,
            storage: [1, 2],
        };
        let other = ArrayVec {
            len: 1,
            storage: [3, 4],
        };
        let _array = read_full_array(&values, &other);
    }
}
