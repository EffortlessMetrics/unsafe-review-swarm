use core::mem::MaybeUninit;

pub fn read_written() -> u32 {
    let mut slot = MaybeUninit::<u32>::uninit();
    slot.write(7);
    // SAFETY: this fixture exposes a same-slot write before assume_init_read.
    unsafe { slot.assume_init_read() }
}

#[cfg(test)]
mod tests {
    use super::read_written;

    #[test]
    fn reads_written() {
        let _value = read_written();
    }
}
