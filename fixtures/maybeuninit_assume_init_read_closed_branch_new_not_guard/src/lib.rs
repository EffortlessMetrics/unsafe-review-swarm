use core::mem::MaybeUninit;

pub fn read_after_closed_new_branch(init: bool) -> u32 {
    let slot = MaybeUninit::<u32>::uninit();
    if init {
        let slot = MaybeUninit::new(7_u32);
        let _ = slot.as_ptr();
    }
    // SAFETY: fixture checks that closed-branch MaybeUninit::new evidence
    // does not initialize the outer slot.
    unsafe { slot.assume_init_read() }
}

#[cfg(test)]
mod tests {
    use super::read_after_closed_new_branch;

    #[test]
    fn mentions_read_after_closed_new_branch() {
        let _ = core::mem::size_of_val(&(read_after_closed_new_branch as fn(bool) -> u32));
    }
}
