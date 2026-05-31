use core::mem::MaybeUninit;

pub fn mutably_borrow_after_other_slot_write(value: u32) {
    let mut slot = MaybeUninit::<u32>::uninit();
    let mut other = MaybeUninit::<u32>::uninit();
    other.write(value);
    // SAFETY: this fixture intentionally initializes a different slot.
    let _value = unsafe { slot.assume_init_mut() };
}

#[cfg(test)]
mod tests {
    use super::mutably_borrow_after_other_slot_write;

    #[test]
    fn mentions_mutably_borrow_after_other_slot_write() {
        let _ = core::mem::size_of_val(&(mutably_borrow_after_other_slot_write as fn(u32)));
    }
}

