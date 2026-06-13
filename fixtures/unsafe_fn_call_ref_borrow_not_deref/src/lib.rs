/// Negative control: `unsafe { getter.get_slot() }` where `get_slot` returns a
/// reference — a call to an unsafe function — must stay `unsafe_fn_call`.
///
/// Even if the call result is borrowed (e.g. `&mut get_slot_result`), the raw
/// data returned by an unsafe function is NOT the same as dereferencing a raw
/// pointer.  The ref-wrapped-deref fix only promotes blocks where `*` appears
/// as the first operator inside the block (`&mut *expr` or `&*expr`).  A block
/// whose content is a bare call expression (no leading `*`) must remain
/// `unsafe_fn_call` regardless of what the callee returns.
pub struct Slot {
    value: u32,
}

impl Slot {
    /// # Safety
    /// No other mutable borrow of `value` must be live when this is called.
    pub unsafe fn get_slot_value(&self) -> u32 {
        self.value
    }
}

pub fn read_slot(slot: &Slot) -> u32 {
    // SAFETY: no aliasing mutable borrows are live.
    // This block is `unsafe { slot.get_slot_value() }` — content does NOT start
    // with `&` followed by `*`, so it must stay `unsafe_fn_call`, not `raw_pointer_deref`.
    unsafe { slot.get_slot_value() }
}
