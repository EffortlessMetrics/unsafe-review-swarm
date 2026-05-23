use core::mem::MaybeUninit;

pub fn assume_written() -> u32 {
    let mut slot = MaybeUninit::<u32>::uninit();
    slot.write(7);
    // SAFETY: this fixture exposes a same-slot write before assume_init.
    unsafe { slot.assume_init() }
}

#[cfg(test)]
mod tests {
    use super::assume_written;

    #[test]
    fn assumes_written() {
        let _value = assume_written();
    }
}
