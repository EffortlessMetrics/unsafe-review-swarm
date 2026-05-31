use core::mem::MaybeUninit;

pub fn read_new_inside_branch(init: bool) -> Option<u32> {
    if init {
        let slot = MaybeUninit::new(7_u32);
        // SAFETY: this branch creates the initialized slot before
        // assume_init_read.
        Some(unsafe { slot.assume_init_read() })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::read_new_inside_branch;

    #[test]
    fn mentions_read_new_inside_branch() {
        let _ = stringify!(read_new_inside_branch);
    }
}
