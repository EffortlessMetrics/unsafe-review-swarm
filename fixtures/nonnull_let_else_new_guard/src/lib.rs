use core::ptr::NonNull;

pub fn expose_nonnull_after_let_else(ptr: *mut u8) -> Option<NonNull<u8>> {
    let Some(_) = NonNull::new(ptr) else {
        return None;
    };
    // SAFETY: NonNull::new returned early on null, so ptr is non-null.
    Some(unsafe { NonNull::new_unchecked(ptr) })
}

#[cfg(test)]
mod tests {
    use super::expose_nonnull_after_let_else;

    #[test]
    fn mentions_expose_nonnull_after_let_else() {
        let _ = stringify!(expose_nonnull_after_let_else);
    }
}
