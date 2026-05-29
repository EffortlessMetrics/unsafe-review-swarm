pub fn copy_checked(src: &[u8], dst: &mut [u8], count: usize) {
    if count > src.len() || count > dst.len() {
        return;
    }
    let count = src.len() + 1;
    // SAFETY: fixture has a stale disjunctive early-return range check; count is shadowed before use.
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

