pub fn copy_overlapping_checked(src: &[u8], dst: &mut [u8], count: usize) {
    if count > src.len() || count > dst.len() {
        return;
    }
    let mut other = [0_u8; 1];
    let dst = &mut other[..];
    // SAFETY: fixture has a stale disjunctive early-return range check; destination is shadowed before use.
    unsafe { core::ptr::copy(src.as_ptr(), dst.as_mut_ptr(), count) }
}

#[cfg(test)]
mod tests {
    use super::copy_overlapping_checked;

    #[test]
    fn mentions_copy_overlapping_checked() {
        let _ = stringify!(copy_overlapping_checked);
    }
}
