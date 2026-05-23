use core::mem::MaybeUninit;

pub fn assume_new() -> u32 {
    let slot = MaybeUninit::new(7_u32);
    // SAFETY: this fixture exposes a same-slot MaybeUninit::new before assume_init.
    unsafe { slot.assume_init() }
}

#[cfg(test)]
mod tests {
    use super::assume_new;

    #[test]
    fn assumes_new() {
        let _value = assume_new();
    }
}
