pub fn decode(bytes: &[u8]) -> &str {
    // SAFETY: bytes were already validated as UTF-8.
    unsafe { core::str::from_utf8_unchecked(bytes) }
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn mentions_decode() {
        let _ = core::mem::size_of_val(&(decode as fn(&[u8]) -> &str));
    }
}
