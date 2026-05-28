use core::ptr::NonNull;

pub fn expose_nonnull_after_null_or_disabled(
    ptr: *mut u8,
    disabled: bool,
) -> Option<NonNull<u8>> {
    if ptr.is_null() || disabled {
        return None;
    }

    // SAFETY: the null-or-disabled guard returned before this construction.
    Some(unsafe { NonNull::new_unchecked(ptr) })
}

#[cfg(test)]
mod tests {
    use super::expose_nonnull_after_null_or_disabled;

    #[test]
    fn mentions_expose_nonnull_after_null_or_disabled() {
        let _ = stringify!(expose_nonnull_after_null_or_disabled);
    }
}
