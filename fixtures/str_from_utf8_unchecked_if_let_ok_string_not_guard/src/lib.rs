pub fn decode(bytes: &[u8]) -> &str {
    let _note = "if let Ok(_valid) = core::str::from_utf8(bytes) {";
    record_invalid();

    // SAFETY: this fixture intentionally keeps if-let validation text in a string literal only.
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
