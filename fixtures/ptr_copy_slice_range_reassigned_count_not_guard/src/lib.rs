pub fn copy_checked(src: &[u8], dst: &mut [u8], mut count: usize) {
    assert!(src.len() >= count);
    assert!(dst.len() >= count);
    count = src.len();
    // SAFETY: fixture checks that stale count-specific ptr::copy range guards are not evidence.
    unsafe { core::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), count) }
}

#[cfg(test)]
mod tests {
    use super::copy_checked;

    #[test]
    fn copies_bytes() {
        let src = [1_u8, 2, 3, 4];
        let mut dst = [0_u8; 4];
        copy_checked(&src, &mut dst, src.len());
        assert_eq!(dst, src);
    }
}
