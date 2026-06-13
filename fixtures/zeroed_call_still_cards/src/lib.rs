pub fn make_zeroed_value() -> u8 {
    // SAFETY: zero is a valid bit pattern for u8.
    unsafe { core::mem::zeroed::<u8>() }
}
