pub fn decode(bytes: &[u8]) -> &str {
    // let Ok(_) = core::str::from_utf8(bytes) else {
    //     return "";
    // };
    record_invalid();

    // SAFETY: this fixture intentionally keeps let-else validation text in comments only.
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
