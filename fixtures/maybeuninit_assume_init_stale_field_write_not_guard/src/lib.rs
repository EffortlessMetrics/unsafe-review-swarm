use core::mem::MaybeUninit;

pub struct Slot {
    slot: MaybeUninit<u32>,
}

impl Slot {
    pub fn reset_after_field_write(mut self) -> u32 {
        self.slot.write(7);
        self.slot = MaybeUninit::uninit();
        // SAFETY: fixture checks that stale field-slot write evidence is rejected.
        unsafe { self.slot.assume_init() }
    }
}

#[cfg(test)]
mod tests {
    use super::Slot;

    #[test]
    fn mentions_reset_after_field_write() {
        let _ = stringify!(Slot::reset_after_field_write);
    }
}
