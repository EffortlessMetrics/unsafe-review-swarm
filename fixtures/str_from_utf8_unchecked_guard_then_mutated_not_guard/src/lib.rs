pub fn decode(bytes: &mut Vec<u8>) -> &str {
    if core::str::from_utf8(bytes).is_err() {
        return "";
    }

    bytes.push(0xff);
    // SAFETY: fixture deliberately mutates the validated buffer before unchecked decode.
    unsafe { core::str::from_utf8_unchecked(bytes) }
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn mentions_decode() {
        let _ = stringify!(decode);
    }
}
