/// A deferred callback stored as an unsafe fn-pointer type.
///
/// `call: unsafe fn(*mut u8)` is a field whose *type* is a fn-pointer to an
/// unsafe function.  It is not itself an unsafe fn declaration and contains no
/// unsafe block.  unsafe-review must not emit a ReviewCard for a fn-pointer
/// type in field/type position.
pub struct Deferred {
    pub call: unsafe fn(*mut u8),
}

/// A container that holds an optional unsafe fn-pointer callback.
/// Again, no unsafe block or unsafe fn declaration here.
pub struct Handler {
    pub f: Option<unsafe fn(usize) -> bool>,
}
