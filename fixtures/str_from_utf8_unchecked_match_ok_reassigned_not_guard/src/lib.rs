pub fn decode_reassigned<'a>(input: &'a [u8], other: &'a [u8]) -> &'a str {
    let mut bytes = input;
    match core::str::from_utf8(bytes) {
        Ok(_) => {
            bytes = other;
            // SAFETY: fixture deliberately invalidates the validated slice before unchecked decode.
            unsafe { core::str::from_utf8_unchecked(bytes) }
        }
        Err(_) => "",
    }
}

#[cfg(test)]
mod tests {
    use super::decode_reassigned;

    #[test]
    fn mentions_decode_reassigned() {
        let _ = stringify!(decode_reassigned);
    }
}
