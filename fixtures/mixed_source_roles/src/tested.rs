//! Test-only helpers living in `src/`. A `#[cfg(test)]` module is not
//! ordinary production code merely because of its path.

#[cfg(test)]
pub mod checks {
    /// Test-only seam: unsafe helper used by unit tests. Visible in PR
    /// mode when changed; test-only never means automatically suppressed.
    pub fn read_test_helper(ptr: *const u8) -> u8 {
        unsafe { *ptr }
    }
}
