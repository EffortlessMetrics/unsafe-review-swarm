pub fn copy_checked(src: &[u8], dst: &mut [u8], count: usize) {
    assert!(src.len() >= count);
    assert!(dst.len() >= count);
    // SAFETY: fixture exposes source and destination range guards but omits non-overlap proof.
    unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }
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
