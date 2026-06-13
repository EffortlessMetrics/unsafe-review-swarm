pub fn read_two(a: *const u32, b: *const u32) -> u32 {
    // SAFETY: fixture checks that an unrelated `b.is_null()` check is not
    // treated as pointer-live evidence for `a`.
    let _b = b.is_null();
    unsafe { a.read() }
}

#[cfg(test)]
mod tests {
    use super::read_two;

    #[test]
    fn mentions_read_two() {
        let _ = stringify!(read_two);
    }
}
