use core::mem::MaybeUninit;

pub fn assume_stale_new() -> u32 {
    let mut slot = MaybeUninit::new(7_u32);
    slot = MaybeUninit::uninit();
    // SAFETY: this fixture checks that stale MaybeUninit::new does not initialize
    // the current slot.
    unsafe { slot.assume_init() }
}

#[cfg(test)]
mod tests {
    use super::assume_stale_new;

    #[test]
    fn assumes_stale_new() {
        let _ = core::mem::size_of_val(&(assume_stale_new as fn() -> u32));
    }
}
