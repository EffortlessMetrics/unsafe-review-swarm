use core::cell::UnsafeCell;

pub struct SharedCell {
    value: UnsafeCell<u32>,
}

// SAFETY: this fixture states the thread-safety promise but provides no Loom witness.
unsafe impl Send for SharedCell {}

/// A longer type whose name contains `SharedCell` as a substring.
/// Tests that mention only `SharedCellar` must NOT credit reach for `SharedCell`.
pub struct SharedCellar {
    value: UnsafeCell<u32>,
}
