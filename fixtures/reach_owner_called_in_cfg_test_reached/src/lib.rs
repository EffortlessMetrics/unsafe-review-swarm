// Positive-control: owner `RawSlice` is referenced inside the
// `#[cfg(test)] mod tests` block.  Test reach must be credited.

pub struct RawSlice {
    ptr: *const u8,
    len: usize,
}

// SAFETY: this fixture states the Send promise but provides no witness.
unsafe impl Send for RawSlice {}

#[cfg(test)]
mod tests {
    use super::RawSlice;

    #[test]
    fn constructs_raw_slice() {
        // RawSlice is mentioned inside the test scope — reach must be credited.
        let _s = RawSlice { ptr: std::ptr::null(), len: 0 };
    }
}
