pub fn decode(bytes: &[u8], split: usize) -> &str {
    if core::str::from_utf8(&bytes[..split]).is_ok() {
        // SAFETY: fixture deliberately validates only a prefix before decoding all bytes.
        unsafe { core::str::from_utf8_unchecked(bytes) }
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decodes_ascii_when_prefix_is_valid() {
        assert_eq!(decode(b"ok", 1), "ok");
    }
}

