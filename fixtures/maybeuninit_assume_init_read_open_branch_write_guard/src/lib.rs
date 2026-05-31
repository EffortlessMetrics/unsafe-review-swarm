use core::mem::MaybeUninit;

pub fn read_write_inside_branch(init: bool) -> Option<u32> {
    let mut slot = MaybeUninit::<u32>::uninit();
    if init {
        slot.write(7);
        // SAFETY: this branch writes the slot before assume_init_read.
        Some(unsafe { slot.assume_init_read() })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::read_write_inside_branch;

    #[test]
    fn mentions_read_write_inside_branch() {
        let _ = stringify!(read_write_inside_branch);
    }
}
