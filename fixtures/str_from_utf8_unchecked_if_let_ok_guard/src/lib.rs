pub fn decode(bytes: &[u8]) -> &str {
    if let Ok(_valid) = core::str::from_utf8(bytes) {
        // SAFETY: the branch above validated this byte slice as UTF-8.
        unsafe { core::str::from_utf8_unchecked(bytes) }
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decodes_ascii() {
        assert_eq!(decode(b"ok"), "ok");
    }
}

