use core::mem::MaybeUninit;

pub fn mutably_borrow_after_stale_new() {
    let mut slot = MaybeUninit::new(7_u32);
    slot = MaybeUninit::uninit();
    // SAFETY: fixture checks that stale MaybeUninit::new evidence does not
    // initialize the current slot before assume_init_mut.
    let _value = unsafe { slot.assume_init_mut() };
}

#[cfg(test)]
mod tests {
    use super::mutably_borrow_after_stale_new;

    #[test]
    fn mentions_mutably_borrow_after_stale_new() {
        let _ = core::mem::size_of_val(&(mutably_borrow_after_stale_new as fn()));
    }
}
