use core::mem::MaybeUninit;

pub fn ref_write_inside_branch(init: bool) -> Option<u32> {
    let mut slot = MaybeUninit::<u32>::uninit();
    if init {
        slot.write(7);
        // SAFETY: this branch writes the slot before assume_init_ref.
        let value = unsafe { slot.assume_init_ref() };
        Some(*value)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::ref_write_inside_branch;

    #[test]
    fn mentions_ref_write_inside_branch() {
        let _ = stringify!(ref_write_inside_branch);
    }
}
