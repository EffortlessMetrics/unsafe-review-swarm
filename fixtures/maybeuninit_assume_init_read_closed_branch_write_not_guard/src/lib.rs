use core::mem::MaybeUninit;

pub fn read_after_closed_branch(init: bool) -> u32 {
    let mut slot = MaybeUninit::<u32>::uninit();
    if init {
        slot.write(7);
    }
    // SAFETY: fixture checks that a closed conditional write is not enough evidence.
    unsafe { slot.assume_init_read() }
}

#[cfg(test)]
mod tests {
    use super::read_after_closed_branch;

    #[test]
    fn mentions_read_after_closed_branch() {
        let _ = core::mem::size_of_val(&(read_after_closed_branch as fn(bool) -> u32));
    }
}
