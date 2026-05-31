use core::mem::MaybeUninit;

pub fn ref_after_new() -> u32 {
    let slot = MaybeUninit::new(7_u32);
    // SAFETY: this fixture exposes a same-slot MaybeUninit::new before
    // assume_init_ref.
    let value = unsafe { slot.assume_init_ref() };
    *value
}

#[cfg(test)]
mod tests {
    use super::ref_after_new;

    #[test]
    fn refs_after_new() {
        let _value = ref_after_new();
    }
}
