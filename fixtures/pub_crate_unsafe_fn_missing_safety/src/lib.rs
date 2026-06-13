pub(crate) unsafe fn in_crate_contract_missing(ptr: *const u8) -> u8 {
    // The missing # Safety section is the fixture target.
    // This is pub(crate), not public API — public_api_surface must be false.
    *ptr
}
