use core::mem::MaybeUninit;

pub fn drop_after_closed_new_branch(init: bool) {
    let mut slot = MaybeUninit::<String>::uninit();
    if init {
        let slot = MaybeUninit::new(String::from("initialized"));
        let _ = slot.as_ptr();
    }
    // SAFETY: fixture checks that closed-branch MaybeUninit::new evidence
    // does not initialize the outer slot.
    unsafe { slot.assume_init_drop() }
}

#[cfg(test)]
mod tests {
    use super::drop_after_closed_new_branch;

    #[test]
    fn mentions_drop_after_closed_new_branch() {
        let _ = core::mem::size_of_val(&(drop_after_closed_new_branch as fn(bool)));
    }
}
