use core::mem::MaybeUninit;

pub fn drop_after_stale_write(value: String) {
    let mut slot = MaybeUninit::<String>::uninit();
    slot.write(value);
    slot = MaybeUninit::uninit();
    // SAFETY: this fixture checks that stale writes do not initialize the current slot.
    unsafe { slot.assume_init_drop() }
}

#[cfg(test)]
mod tests {
    use super::drop_after_stale_write;

    #[test]
    fn mentions_drop_after_stale_write() {
        let _ = core::mem::size_of_val(&(drop_after_stale_write as fn(String)));
    }
}
