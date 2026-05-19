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

pub fn read_full_array<T: Copy, const CAP: usize>(values: &ArrayVec<T, CAP>) -> [T; CAP] {
    let full = values.len() == values.capacity();
    observe(full);
    let ptr = values.as_ptr();
    // SAFETY: this fixture intentionally observes len/capacity without enforcing it.
    unsafe { core::ptr::read(ptr as *const [T; CAP]) }
}

fn observe(_full: bool) {}

#[cfg(test)]
mod tests {
    use super::{read_full_array, ArrayVec};

    #[test]
    fn reaches_read_full_array() {
        let values = ArrayVec {
            len: 2,
            storage: [1, 2],
        };
        let _array = read_full_array(&values);
    }
}
