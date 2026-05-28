use core::ptr::NonNull;

pub fn expose_nonnull_if_non_null(ptr: *mut u8) -> Option<NonNull<u8>> {
    if !ptr.is_null() {
        // SAFETY: the open branch only constructs NonNull when ptr is not null.
        Some(unsafe { NonNull::new_unchecked(ptr) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::expose_nonnull_if_non_null;

    #[test]
    fn mentions_expose_nonnull_if_non_null() {
        let _ = stringify!(expose_nonnull_if_non_null);
    }
}

