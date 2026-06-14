use core::sync::atomic::{AtomicUsize, Ordering};

pub struct Handle(*mut u8);

impl Handle {
    // F3-2 negative control: `from_ptr(` and `fetch_or(` both appear on the
    // same diff line, but `fetch_or` is NOT nested inside `from_ptr`'s argument
    // list — they are separate sub-expressions.  The old line-co-occurrence
    // heuristic would have flagged this as atomic_pointer_state; the same-call
    // binding fix must not produce a card here.
    pub fn not_same_call(raw: *mut u8, flags: &AtomicUsize) -> Self {
        let _ = flags.fetch_or(1, Ordering::Relaxed); Handle::from_ptr(raw)
    }

    fn from_ptr(p: *mut u8) -> Self {
        Self(p)
    }
}
