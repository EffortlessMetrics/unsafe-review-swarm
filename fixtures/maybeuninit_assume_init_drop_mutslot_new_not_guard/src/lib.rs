use core::mem::MaybeUninit;

pub fn drop_after_prefixed_slot() {
    let mutslot = MaybeUninit::new(String::from("initialized"));
    let mut slot = MaybeUninit::<String>::uninit();
    let _ = mutslot;
    // SAFETY: fixture intentionally initializes `mutslot`, not `slot`, before
    // assume_init_drop.
    unsafe { slot.assume_init_drop() }
}

#[cfg(test)]
mod tests {
    use super::drop_after_prefixed_slot;

    #[test]
    fn mentions_drop_after_prefixed_slot() {
        let _ = core::mem::size_of_val(&(drop_after_prefixed_slot as fn()));
    }
}
