pub fn decode_shadowed(input: &[u8], fallback: &[u8]) -> Result<&str, core::str::Utf8Error> {
    let bytes = input;
    core::str::from_utf8(bytes)?;
    let bytes = fallback;
    // SAFETY: fixture deliberately shadows the validated slice before unchecked decode.
    Ok(unsafe { core::str::from_utf8_unchecked(bytes) })
}

#[cfg(test)]
mod tests {
    use super::decode_shadowed;

    #[test]
    fn mentions_decode_shadowed() {
        let _ = stringify!(decode_shadowed);
    }
}
