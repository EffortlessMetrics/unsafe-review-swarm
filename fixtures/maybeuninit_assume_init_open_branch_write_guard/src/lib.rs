use core::mem::MaybeUninit;

pub fn assume_inside_branch(init: bool) -> Option<u32> {
    let mut slot = MaybeUninit::<u32>::uninit();
    if init {
        slot.write(7);
        // SAFETY: this branch writes the slot before assuming it is initialized.
        Some(unsafe { slot.assume_init() })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::assume_inside_branch;

    #[test]
    fn mentions_assume_inside_branch() {
        let _ = stringify!(assume_inside_branch);
    }
}
