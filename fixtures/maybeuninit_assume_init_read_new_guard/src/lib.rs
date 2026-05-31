use core::mem::MaybeUninit;

pub fn read_after_new() -> u32 {
    let slot = MaybeUninit::new(7_u32);
    // SAFETY: this fixture exposes a same-slot MaybeUninit::new before
    // assume_init_read.
    unsafe { slot.assume_init_read() }
}

#[cfg(test)]
mod tests {
    use super::read_after_new;

    #[test]
    fn reads_after_new() {
        let _value = read_after_new();
    }
}
