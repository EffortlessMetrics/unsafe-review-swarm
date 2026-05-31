use core::mem::MaybeUninit;

pub fn mutate_after_closed_branch(init: bool) -> u32 {
    let mut slot = MaybeUninit::<u32>::uninit();
    if init {
        slot.write(7);
    }
    // SAFETY: fixture checks that a closed conditional write is not enough evidence.
    let value = unsafe { slot.assume_init_mut() };
    *value += 1;
    *value
}

#[cfg(test)]
mod tests {
    use super::mutate_after_closed_branch;

    #[test]
    fn mentions_mutate_after_closed_branch() {
        let _ = core::mem::size_of_val(&(mutate_after_closed_branch as fn(bool) -> u32));
    }
}
