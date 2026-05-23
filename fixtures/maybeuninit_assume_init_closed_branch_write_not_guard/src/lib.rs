use core::mem::MaybeUninit;

pub fn assume_after_closed_branch(init: bool) -> u32 {
    let mut slot = MaybeUninit::<u32>::uninit();
    if init {
        slot.write(7);
    }
    // SAFETY: fixture checks that a closed conditional write is not enough evidence.
    unsafe { slot.assume_init() }
}

#[cfg(test)]
mod tests {
    use super::assume_after_closed_branch;

    #[test]
    fn mentions_assume_after_closed_branch() {
        let _ = stringify!(assume_after_closed_branch);
    }
}
