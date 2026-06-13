/// Unsafe fn that advances a raw pointer by an index — must still produce a card.
///
/// # Safety
/// Caller must ensure `ptr` is valid for reads up to `i` elements beyond its base.
pub unsafe fn advance(ptr: *const u8, i: usize) -> *const u8 {
    ptr.add(i)
}
