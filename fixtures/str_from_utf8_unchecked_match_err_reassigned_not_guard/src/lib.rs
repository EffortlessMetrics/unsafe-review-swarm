pub fn decode_reassigned<'a>(
    input: &'a [u8],
    other: &'a [u8],
) -> Result<&'a str, core::str::Utf8Error> {
    let mut bytes = input;
    match core::str::from_utf8(bytes) {
        Ok(_) => {}
        Err(err) => return Err(err),
    }

    bytes = other;
    // SAFETY: fixture deliberately invalidates the validated slice before unchecked decode.
    Ok(unsafe { core::str::from_utf8_unchecked(bytes) })
}

#[cfg(test)]
mod tests {
    use super::decode_reassigned;

    #[test]
    fn mentions_decode_reassigned() {
        let _ = stringify!(decode_reassigned);
    }
}
