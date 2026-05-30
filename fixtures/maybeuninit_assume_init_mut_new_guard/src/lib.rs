use core::mem::MaybeUninit;

pub fn assume_mut_new() -> u32 {
    let mut slot = MaybeUninit::new(7_u32);
    // SAFETY: this fixture exposes a same-slot mutable MaybeUninit::new binding.
    unsafe { slot.assume_init() }
}

#[cfg(test)]
mod tests {
    use super::assume_mut_new;

    #[test]
    fn assumes_mut_new() {
        let _value = assume_mut_new();
    }
}
