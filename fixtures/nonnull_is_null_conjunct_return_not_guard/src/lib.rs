use core::ptr::NonNull;

pub fn expose_nonnull_after_partial_null_return(
    ptr: *mut u8,
    disabled: bool,
) -> Option<NonNull<u8>> {
    if ptr.is_null() && disabled {
        return None;
    }

    // SAFETY: this fixture intentionally permits the unchecked path when ptr is
    // null but disabled is false.
    Some(unsafe { NonNull::new_unchecked(ptr) })
}

#[cfg(test)]
mod tests {
    use super::expose_nonnull_after_partial_null_return;

    #[test]
    fn mentions_expose_nonnull_after_partial_null_return() {
        let _ = stringify!(expose_nonnull_after_partial_null_return);
    }
}
