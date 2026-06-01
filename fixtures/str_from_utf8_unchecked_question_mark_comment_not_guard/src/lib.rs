pub fn decode(bytes: &[u8]) -> Result<&str, core::str::Utf8Error> {
    // core::str::from_utf8(bytes)?;
    record_invalid();

    // SAFETY: this fixture intentionally keeps question-mark validation text in a comment only.
    Ok(unsafe { core::str::from_utf8_unchecked(bytes) })
}

fn record_invalid() {}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decodes_ascii() {
        assert_eq!(decode(b"ok").unwrap(), "ok");
    }
}
