use core::ptr::NonNull;

pub fn expose_nonnull_when_ready(
    ptr: *mut u8,
    allow_reviewed_path: bool,
) -> Option<NonNull<u8>> {
    if !ptr.is_null() && allow_reviewed_path {
        // SAFETY: the open branch only constructs NonNull when ptr is not null.
        Some(unsafe { NonNull::new_unchecked(ptr) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::expose_nonnull_when_ready;

    #[test]
    fn mentions_expose_nonnull_when_ready() {
        let _ = stringify!(expose_nonnull_when_ready);
    }
}
