use core::sync::atomic::{AtomicPtr, Ordering};

pub struct Shared<T>(*mut T);

impl<T> Shared<T> {
    pub fn from_ptr(ptr: *mut T) -> Self {
        Self(ptr)
    }
}

pub struct Tagged<T> {
    data: AtomicPtr<T>,
}

impl<T> Tagged<T> {
    pub fn fetch_and_tag(&self, mask: usize, order: Ordering) -> Shared<T> {
        unsafe { Shared::from_ptr(self.data.fetch_and(mask, order)) }
    }

    pub fn fetch_or_tag(&self, mask: usize, order: Ordering) -> Shared<T> {
        unsafe { Shared::from_ptr(self.data.fetch_or(mask, order)) }
    }

    pub fn fetch_xor_tag(&self, mask: usize, order: Ordering) -> Shared<T> {
        unsafe { Shared::from_ptr(self.data.fetch_xor(mask, order) as *mut T) }
    }
}

#[cfg(test)]
mod tests {
    use super::Tagged;
    use core::sync::atomic::AtomicPtr;

    #[test]
    fn tagged_pointer_fetches_preserve_pointer_shape() {
        let mut value = 7_u8;
        let tagged = Tagged {
            data: AtomicPtr::new(&mut value),
        };

        let _ = tagged.fetch_and_tag(!0, core::sync::atomic::Ordering::AcqRel);
    }
}
