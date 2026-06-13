pub fn read_checked(a: *const u32) -> Option<u32> {
    // SAFETY: fixture validates that a dominating same-receiver `!a.is_null()`
    // open-branch guard discharges the pointer-live obligation for `a.read()`.
    if !a.is_null() {
        Some(unsafe { a.read() })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::read_checked;

    #[test]
    fn mentions_read_checked() {
        let _ = stringify!(read_checked);
    }
}
