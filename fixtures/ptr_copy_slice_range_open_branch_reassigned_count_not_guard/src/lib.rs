pub fn copy_overlapping_checked(src: &[u8], dst: &mut [u8], mut count: usize) {
    if src.len() >= count {
        if dst.len() >= count {
            count = 0;
            // SAFETY: fixture checks that stale open-branch count guards are not ptr::copy range evidence.
            unsafe { core::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), count) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::copy_overlapping_checked;

    #[test]
    fn leaves_destination_when_count_resets() {
        let src = [1_u8, 2, 3, 4];
        let mut dst = [0_u8; 4];
        copy_overlapping_checked(&src, &mut dst, src.len());
        assert_eq!(dst, [0_u8; 4]);
    }
}
