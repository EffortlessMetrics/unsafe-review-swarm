pub fn copy_checked(mut src: &[u8], dst: &mut [u8], count: usize) {
    if src.len() >= count {
        if dst.len() >= count {
            let alternate = [9_u8, 8, 7, 6];
            src = &alternate;
            // SAFETY: fixture checks that stale open-branch source guards are not range evidence.
            unsafe { core::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), count) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::copy_checked;

    #[test]
    fn copies_from_alternate_source() {
        let src = [1_u8, 2, 3, 4];
        let mut dst = [0_u8; 4];
        copy_checked(&src, &mut dst, src.len());
        assert_eq!(dst, [9_u8, 8, 7, 6]);
    }
}
