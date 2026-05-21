pub fn shift_with_other_len(bytes: &mut [u8], count: usize, other: &[u8]) {
    assert!(other.len() >= count);
    let src = bytes.as_ptr().wrapping_add(1);
    let dst = bytes.as_mut_ptr();
    // SAFETY: fixture checks that an unrelated slice length is not ptr::copy range evidence.
    unsafe { core::ptr::copy(src, dst, count) }
}

#[cfg(test)]
mod tests {
    use super::shift_with_other_len;

    #[test]
    fn shifts_bytes() {
        let mut bytes = [1_u8, 2, 3, 4];
        let other = [0_u8; 3];
        shift_with_other_len(&mut bytes, 3, &other);
        assert_eq!(bytes, [2, 3, 4, 4]);
    }
}
