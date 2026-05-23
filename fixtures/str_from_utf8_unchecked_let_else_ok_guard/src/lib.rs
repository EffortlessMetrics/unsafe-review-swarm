pub fn decode(bytes: &[u8]) -> &str {
    let Ok(_) = core::str::from_utf8(bytes) else {
        return "";
    };

    // SAFETY: the let-else above returns before this point when the same bytes are not UTF-8.
    unsafe { core::str::from_utf8_unchecked(bytes) }
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decodes_ascii() {
        assert_eq!(decode(b"ok"), "ok");
    }
}
