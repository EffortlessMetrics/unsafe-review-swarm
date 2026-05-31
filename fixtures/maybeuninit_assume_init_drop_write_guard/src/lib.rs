use core::mem::MaybeUninit;

pub fn drop_written(value: String) {
    let mut slot = MaybeUninit::<String>::uninit();
    slot.write(value);
    // SAFETY: this fixture exposes a same-slot write before assume_init_drop.
    unsafe { slot.assume_init_drop() }
}

#[cfg(test)]
mod tests {
    use super::drop_written;

    #[test]
    fn drops_written() {
        drop_written(String::from("initialized"));
    }
}
