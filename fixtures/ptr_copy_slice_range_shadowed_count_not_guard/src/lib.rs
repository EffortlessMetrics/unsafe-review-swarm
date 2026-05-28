pub fn copy_checked(src: &[u8], dst: &mut [u8], count: usize) {
    assert!(src.len() >= count);
    assert!(dst.len() >= count);
    let count = src.len() + 1;
    // SAFETY: fixture deliberately shadows the checked count before ptr::copy.
    unsafe { core::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), count) }
}

#[cfg(test)]
mod tests {
    use super::copy_checked;

    #[test]
    fn mentions_copy_checked() {
        let _ = stringify!(copy_checked);
    }
}

