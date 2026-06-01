pub fn decode_shadowed<'a>(
    bytes: &'a [u8],
    fallback: &'a [u8],
) -> Result<&'a str, core::str::Utf8Error> {
    match core::str::from_utf8(bytes) {
        Ok(_) => {}
        Err(err) => return Err(err),
    }

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
