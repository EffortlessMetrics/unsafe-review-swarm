/// A safe free function named `unreachable_unchecked`. Calling it outside any
/// unsafe scope must not produce a ReviewCard (D1 unsafe-scope gate
/// distinguishes it from `core::hint::unreachable_unchecked`).

/// Safe `unreachable_unchecked` homonym — returns a sentinel, no unsafe involved.
pub fn unreachable_unchecked() -> u32 {
    0
}

/// Safe caller invoking the safe `unreachable_unchecked` — must not produce a card.
pub fn sentinel() -> u32 {
    unreachable_unchecked()
}
