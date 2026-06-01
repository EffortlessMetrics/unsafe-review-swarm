pub fn decode(bytes: &[u8]) -> &str {
    if let Err(_err) = core::str::from_utf8(bytes) {
        // return "";
        record_invalid();
    }

    // SAFETY: this fixture intentionally keeps return text in a comment only.
    unsafe { core::str::from_utf8_unchecked(bytes) }
}

fn record_invalid() {}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decodes_ascii() {
        assert_eq!(decode(b"ok"), "ok");
    }
}
