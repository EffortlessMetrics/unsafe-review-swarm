use core::mem::MaybeUninit;

pub fn read_stale() -> u32 {
    let mut slot = MaybeUninit::<u32>::uninit();
    slot.write(7);
    slot = MaybeUninit::uninit();
    // SAFETY: this fixture checks that stale writes do not initialize the current slot.
    unsafe { slot.assume_init_read() }
}

#[cfg(test)]
mod tests {
    use super::read_stale;

    #[test]
    fn reads_stale() {
        let _ = core::mem::size_of_val(&(read_stale as fn() -> u32));
    }
}
