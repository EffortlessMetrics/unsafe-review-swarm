pub fn copy_overlapping_checked(src: &[u8], dst: &mut [u8], mut count: usize) {
    if src.len() >= count {
        if dst.len() >= count {
            count += src.len();
            // SAFETY: fixture checks that stale open-branch count guards are not ptr::copy range evidence.
            unsafe { core::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), count) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::copy_overlapping_checked;

    #[test]
    fn copies_bytes_after_count_mutation() {
        let src = [1_u8, 2, 3, 4];
        let mut dst = [0_u8; 4];
        copy_overlapping_checked(&src, &mut dst, 0);
        assert_eq!(dst, src);
    }
}
