use core::mem::MaybeUninit;

pub fn mutably_borrow_shadowed() {
    let mut slot = MaybeUninit::<u32>::uninit();
    slot.write(7);
    let mut slot = MaybeUninit::<u32>::uninit();
    // SAFETY: this fixture checks that shadowed writes do not initialize the current slot.
    let _value = unsafe { slot.assume_init_mut() };
}

#[cfg(test)]
mod tests {
    use super::mutably_borrow_shadowed;

    #[test]
    fn mutably_borrows_shadowed() {
        let _ = core::mem::size_of_val(&(mutably_borrow_shadowed as fn()));
    }
}
