use core::ptr::NonNull;

pub fn expose_nonnull(ptr: *mut u8) -> Option<NonNull<u8>> {
    if ptr.is_null() {
        // return None
        record_null();
    }
    // SAFETY: this fixture intentionally keeps return text in a comment only.
    Some(unsafe { NonNull::new_unchecked(ptr) })
}

fn record_null() {}

#[cfg(test)]
mod tests {
    use super::expose_nonnull;

    #[test]
    fn mentions_expose_nonnull() {
        let _ = stringify!(expose_nonnull);
    }
}
