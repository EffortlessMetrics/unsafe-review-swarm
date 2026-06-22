//! D1 negative control: a type with a safe `new_unchecked`-named constructor.
//! `new_unchecked` is a common safe constructor name (it also spells
//! `NonNull::new_unchecked`, `Option::unwrap_unchecked`, etc.); calling a safe
//! homonym on a line containing "Pin" outside any unsafe scope must not produce
//! a ReviewCard. The D1 unsafe-scope gate distinguishes the unsafe stdlib
//! `Pin::new_unchecked` call from safe homonyms.

/// A type that happens to expose a safe `new_unchecked` constructor. It is not
/// `Pin` and involves no `unsafe`.
pub struct Slot {
    inner: u32,
}

impl Slot {
    /// Safe constructor — no unsafe involved. The name collides with
    /// `Pin::new_unchecked` at the bare-call-name level, which is exactly the
    /// D1 homonym the unsafe-scope gate must reject.
    pub fn new_unchecked(inner: u32) -> Self {
        Slot { inner }
    }

    pub fn inner(&self) -> u32 {
        self.inner
    }
}

/// Safe caller invoking the safe `Slot::new_unchecked` on a line that also
/// mentions "Pin" in a comment — must not produce a card.
// Note: "Pin" appears in this comment to stress the D4 comment/string masking
// alongside the D1 unsafe-scope gate. The bare `new_unchecked` call name plus
// the "Pin" token must still not card because the call is outside unsafe scope.
pub fn make_slot() -> Slot {
    Slot::new_unchecked(7)
}
