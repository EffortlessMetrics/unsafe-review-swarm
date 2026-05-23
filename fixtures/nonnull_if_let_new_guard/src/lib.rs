use core::ptr::NonNull;

pub fn expose_nonnull_after_if_let(ptr: *mut u8) -> Option<NonNull<u8>> {
    if let Some(_) = NonNull::new(ptr) {
        // SAFETY: NonNull::new returned Some above, so ptr is non-null.
        Some(unsafe { NonNull::new_unchecked(ptr) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::expose_nonnull_after_if_let;

    #[test]
    fn mentions_expose_nonnull_after_if_let() {
        let _ = stringify!(expose_nonnull_after_if_let);
    }
}
