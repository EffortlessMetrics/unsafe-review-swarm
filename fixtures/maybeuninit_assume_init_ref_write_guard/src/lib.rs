use core::mem::MaybeUninit;

pub fn borrow_written() -> u32 {
    let mut slot = MaybeUninit::<u32>::uninit();
    slot.write(7);
    // SAFETY: this fixture exposes a same-slot write before assume_init_ref.
    let value = unsafe { slot.assume_init_ref() };
    *value
}

#[cfg(test)]
mod tests {
    use super::borrow_written;

    #[test]
    fn borrows_written() {
        let _value = borrow_written();
    }
}
