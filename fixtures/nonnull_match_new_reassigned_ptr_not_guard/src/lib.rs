use core::ptr::NonNull;

pub fn expose_nonnull_after_stale_match(
    mut ptr: *mut u8,
    other: *mut u8,
) -> Option<NonNull<u8>> {
    match NonNull::new(ptr) {
        Some(_) => {
            ptr = other;
            // SAFETY: this fixture intentionally changes the checked pointer before use.
            Some(unsafe { NonNull::new_unchecked(ptr) })
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::expose_nonnull_after_stale_match;

    #[test]
    fn mentions_expose_nonnull_after_stale_match() {
        let _ = stringify!(expose_nonnull_after_stale_match);
    }
}
