use core::mem::MaybeUninit;

pub fn assume_after_closed_new_branch(init: bool) -> u32 {
    let slot = MaybeUninit::<u32>::uninit();
    if init {
        let slot = MaybeUninit::new(7_u32);
        let _ = slot.as_ptr();
    }
    // SAFETY: this fixture checks that closed-branch MaybeUninit::new evidence
    // does not initialize the outer slot.
    unsafe { slot.assume_init() }
}

#[cfg(test)]
mod tests {
    use super::assume_after_closed_new_branch;

    #[test]
    fn mentions_assume_after_closed_new_branch() {
        let _ = stringify!(assume_after_closed_new_branch);
    }
}
