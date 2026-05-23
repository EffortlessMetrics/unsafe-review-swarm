pub fn decode(bytes: &[u8]) -> &str {
    if let Err(_err) = core::str::from_utf8(bytes) {
        return "";
    }

    // SAFETY: the if-let above returns before this point when the same bytes are not UTF-8.
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
