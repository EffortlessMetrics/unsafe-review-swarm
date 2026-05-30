use core::mem::MaybeUninit;

pub fn assume_after_prefixed_slot() -> u32 {
    let mutslot = MaybeUninit::new(7_u32);
    let slot = MaybeUninit::<u32>::uninit();
    let _ = mutslot;
    // SAFETY: this fixture intentionally initializes `mutslot`, not `slot`.
    unsafe { slot.assume_init() }
}

#[cfg(test)]
mod tests {
    use super::assume_after_prefixed_slot;

    #[test]
    fn mentions_prefixed_slot() {
        let _ = core::mem::size_of_val(&(assume_after_prefixed_slot as fn() -> u32));
    }
}
