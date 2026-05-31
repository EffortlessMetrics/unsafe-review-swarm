use core::mem::MaybeUninit;

pub fn mut_write_inside_branch(init: bool) -> Option<u32> {
    let mut slot = MaybeUninit::<u32>::uninit();
    if init {
        slot.write(7);
        // SAFETY: this branch writes the slot before assume_init_mut.
        let value = unsafe { slot.assume_init_mut() };
        *value += 1;
        Some(*value)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::mut_write_inside_branch;

    #[test]
    fn mentions_mut_write_inside_branch() {
        let _ = stringify!(mut_write_inside_branch);
    }
}
