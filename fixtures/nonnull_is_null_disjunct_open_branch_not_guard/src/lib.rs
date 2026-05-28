use core::ptr::NonNull;

pub fn expose_nonnull_when_allowed(
    ptr: *mut u8,
    allow_unchecked: bool,
) -> Option<NonNull<u8>> {
    if !ptr.is_null() || allow_unchecked {
        // SAFETY: this fixture intentionally permits the unchecked path without
        // same-pointer nullability evidence when allow_unchecked is true.
        Some(unsafe { NonNull::new_unchecked(ptr) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::expose_nonnull_when_allowed;

    #[test]
    fn mentions_expose_nonnull_when_allowed() {
        let _ = stringify!(expose_nonnull_when_allowed);
    }
}
