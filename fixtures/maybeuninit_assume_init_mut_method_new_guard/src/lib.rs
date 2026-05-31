use core::mem::MaybeUninit;

pub fn mut_after_new() -> u32 {
    let mut slot = MaybeUninit::new(7_u32);
    // SAFETY: this fixture exposes a same-slot MaybeUninit::new before
    // assume_init_mut.
    let value = unsafe { slot.assume_init_mut() };
    *value += 1;
    *value
}

#[cfg(test)]
mod tests {
    use super::mut_after_new;

    #[test]
    fn muts_after_new() {
        let _value = mut_after_new();
    }
}
