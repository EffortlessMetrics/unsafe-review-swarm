use core::mem::MaybeUninit;

pub fn mutably_borrow_after_stale_write() {
    let mut slot = MaybeUninit::<u32>::uninit();
    slot.write(7);
    slot = MaybeUninit::uninit();
    // SAFETY: this fixture checks that stale writes do not initialize the current slot.
    let _value = unsafe { slot.assume_init_mut() };
}

#[cfg(test)]
mod tests {
    use super::mutably_borrow_after_stale_write;

    #[test]
    fn mentions_mutably_borrow_after_stale_write() {
        let _ = core::mem::size_of_val(&(mutably_borrow_after_stale_write as fn()));
    }
}
