//! Fixture input under `tests/fixtures/`: intentional unsafe shape used
//! as test data, not shipped production code.

/// Fixture seam: deliberate unchecked read exercised by integration tests.
pub fn fixture_read(ptr: *const u8) -> u8 {
    unsafe { *ptr }
}
