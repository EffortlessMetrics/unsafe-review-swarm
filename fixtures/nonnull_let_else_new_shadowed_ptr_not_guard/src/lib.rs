use core::ptr::NonNull;

pub fn expose_nonnull_after_shadowed_let_else(
    ptr: *mut u8,
    other: *mut u8,
) -> Option<NonNull<u8>> {
    let Some(_) = NonNull::new(ptr) else {
        return None;
    };
    let ptr = other;
    // SAFETY: this fixture intentionally shadows the checked pointer before use.
    Some(unsafe { NonNull::new_unchecked(ptr) })
}

#[cfg(test)]
mod tests {
    use super::expose_nonnull_after_shadowed_let_else;

    #[test]
    fn mentions_expose_nonnull_after_shadowed_let_else() {
        let _ = stringify!(expose_nonnull_after_shadowed_let_else);
    }
}
