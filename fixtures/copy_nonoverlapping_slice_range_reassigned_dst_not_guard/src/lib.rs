pub fn copy_checked(src: &[u8], mut dst: &mut [u8], count: usize, fallback: &mut [u8]) {
    assert!(src.len() >= count);
    assert!(dst.len() >= count);
    dst = fallback;
    // SAFETY: fixture checks that a stale destination slice length guard is not range evidence.
    unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }
}

#[cfg(test)]
mod tests {
    use super::copy_checked;

    #[test]
    fn copies_bytes() {
        let src = [1_u8, 2, 3, 4];
        let mut dst = [0_u8; 4];
        let mut fallback = [0_u8; 4];
        copy_checked(&src, &mut dst, src.len(), &mut fallback);
        assert_eq!(fallback, src);
    }
}

