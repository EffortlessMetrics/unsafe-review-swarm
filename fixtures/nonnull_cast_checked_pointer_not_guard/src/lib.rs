use core::ptr::NonNull;

pub fn expose_cast_nonnull(ptr: *mut u8) -> Option<NonNull<u16>> {
    NonNull::new(ptr)?;
    // SAFETY: fixture checks that pre-cast nullability evidence is not treated as same-target
    // evidence for the cast expression passed to NonNull::new_unchecked.
    Some(unsafe { NonNull::new_unchecked(ptr.cast::<u16>()) })
}

#[cfg(test)]
mod tests {
    use super::expose_cast_nonnull;

    #[test]
    fn mentions_expose_cast_nonnull() {
        let _ = stringify!(expose_cast_nonnull);
    }
}
