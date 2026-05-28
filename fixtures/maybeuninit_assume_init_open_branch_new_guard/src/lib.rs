use core::mem::MaybeUninit;

pub fn assume_new_inside_branch(init: bool) -> Option<u32> {
    if init {
        let slot = MaybeUninit::new(7_u32);
        // SAFETY: this branch creates the initialized slot before assume_init.
        Some(unsafe { slot.assume_init() })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::assume_new_inside_branch;

    #[test]
    fn mentions_assume_new_inside_branch() {
        let _ = stringify!(assume_new_inside_branch);
    }
}
