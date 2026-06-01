pub fn decode_reassigned<'a>(input: &'a [u8], fallback: &'a [u8]) -> &'a str {
    let mut bytes = input;
    if core::str::from_utf8(bytes).is_err() {
        return "";
    }

    bytes = fallback;
    // SAFETY: fixture deliberately invalidates the validated slice before unchecked decode.
    unsafe { core::str::from_utf8_unchecked(bytes) }
}

#[cfg(test)]
mod tests {
    use super::decode_reassigned;

    #[test]
    fn mentions_decode_reassigned() {
        let _ = stringify!(decode_reassigned);
    }
}
