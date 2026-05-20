pub fn copy_overlapping_checked(src: &[u8], mut dst: &mut [u8], count: usize) -> [u8; 4] {
    let mut alternate = [0_u8; 4];
    if src.len() >= count {
        if dst.len() >= count {
            dst = &mut alternate;
            // SAFETY: fixture checks that stale open-branch destination guards are not ptr::copy range evidence.
            unsafe { core::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), count) }
        }
    }
    alternate
}

#[cfg(test)]
mod tests {
    use super::copy_overlapping_checked;

    #[test]
    fn copies_into_alternate_destination() {
        let src = [1_u8, 2, 3, 4];
        let mut dst = [0_u8; 4];
        let alternate = copy_overlapping_checked(&src, &mut dst, src.len());
        assert_eq!(dst, [0_u8; 4]);
        assert_eq!(alternate, src);
    }
}
