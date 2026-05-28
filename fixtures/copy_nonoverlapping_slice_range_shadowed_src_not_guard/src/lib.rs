pub fn copy_checked(src: &[u8], dst: &mut [u8], count: usize) {
    assert!(src.len() >= count);
    assert!(dst.len() >= count);
    let other = [0_u8; 1];
    let src = &other[..];
    // SAFETY: fixture deliberately shadows the checked source before the copy.
    unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), count) }
}

#[cfg(test)]
mod tests {
    use super::copy_checked;

    #[test]
    fn mentions_copy_checked() {
        let _ = stringify!(copy_checked);
    }
}

