use core::mem::MaybeUninit;

pub fn borrow_shadowed() {
    let mut slot = MaybeUninit::<u32>::uninit();
    slot.write(7);
    let slot = MaybeUninit::<u32>::uninit();
    // SAFETY: this fixture checks that shadowed writes do not initialize the current slot.
    let _value = unsafe { slot.assume_init_ref() };
}

#[cfg(test)]
mod tests {
    use super::borrow_shadowed;

    #[test]
    fn borrows_shadowed() {
        let _ = core::mem::size_of_val(&(borrow_shadowed as fn()));
    }
}
