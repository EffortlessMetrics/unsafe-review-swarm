use core::mem::MaybeUninit;

pub fn drop_after_closed_branch(init: bool, value: String) {
    let mut slot = MaybeUninit::<String>::uninit();
    if init {
        slot.write(value);
    }
    // SAFETY: fixture checks that a closed conditional write is not enough evidence.
    unsafe { slot.assume_init_drop() }
}

#[cfg(test)]
mod tests {
    use super::drop_after_closed_branch;

    #[test]
    fn mentions_drop_after_closed_branch() {
        let _ = core::mem::size_of_val(&(drop_after_closed_branch as fn(bool, String)));
    }
}
