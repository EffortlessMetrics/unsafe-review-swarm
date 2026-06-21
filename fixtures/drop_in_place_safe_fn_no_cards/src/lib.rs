/// A safe free function named `drop_in_place`. Calling it outside any unsafe
/// scope must not produce a ReviewCard (D1 unsafe-scope gate distinguishes it
/// from `core::ptr::drop_in_place`).
pub struct Slot {
    live: bool,
}

/// Safe `drop_in_place` homonym — just flips a flag, no unsafe involved.
pub fn drop_in_place(slot: &mut Slot) {
    slot.live = false;
}

/// Safe caller invoking the safe `drop_in_place` — must not produce a card.
pub fn release(slot: &mut Slot) {
    drop_in_place(slot)
}
