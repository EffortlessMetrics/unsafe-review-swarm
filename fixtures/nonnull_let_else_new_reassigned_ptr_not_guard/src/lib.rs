use core::ptr::NonNull;

pub fn expose_nonnull_after_stale_let_else(
    mut ptr: *mut u8,
    other: *mut u8,
) -> Option<NonNull<u8>> {
    let Some(_) = NonNull::new(ptr) else {
        return None;
    };
    ptr = other;
    // SAFETY: this fixture intentionally changes the checked pointer before use.
    Some(unsafe { NonNull::new_unchecked(ptr) })
}

#[cfg(test)]
mod tests {
    use super::expose_nonnull_after_stale_let_else;

    #[test]
    fn mentions_expose_nonnull_after_stale_let_else() {
        let _ = stringify!(expose_nonnull_after_stale_let_else);
    }
}
