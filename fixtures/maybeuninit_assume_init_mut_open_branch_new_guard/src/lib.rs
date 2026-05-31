use core::mem::MaybeUninit;

pub fn mut_new_inside_branch(init: bool) -> Option<u32> {
    if init {
        let mut slot = MaybeUninit::new(7_u32);
        // SAFETY: this branch creates the initialized slot before
        // assume_init_mut.
        let value = unsafe { slot.assume_init_mut() };
        *value += 1;
        Some(*value)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::mut_new_inside_branch;

    #[test]
    fn mentions_mut_new_inside_branch() {
        let _ = stringify!(mut_new_inside_branch);
    }
}
