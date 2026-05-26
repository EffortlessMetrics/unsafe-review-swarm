use core::ptr::NonNull;

pub fn expose_nonnull_after_stale_is_null(
    mut ptr: *mut u8,
    other: *mut u8,
) -> Option<NonNull<u8>> {
    if ptr.is_null() {
        return None;
    }
    ptr = other;
    // SAFETY: this fixture intentionally changes the checked pointer before use.
    Some(unsafe { NonNull::new_unchecked(ptr) })
}

#[cfg(test)]
mod tests {
    use super::expose_nonnull_after_stale_is_null;

    #[test]
    fn mentions_expose_nonnull_after_stale_is_null() {
        let _ = stringify!(expose_nonnull_after_stale_is_null);
    }
}
