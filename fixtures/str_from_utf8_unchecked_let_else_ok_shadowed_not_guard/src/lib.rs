pub fn decode_shadowed<'a>(bytes: &'a [u8], fallback: &'a [u8]) -> &'a str {
    let Ok(_) = core::str::from_utf8(bytes) else {
        return "";
    };

    let bytes = fallback;
    // SAFETY: fixture deliberately shadows the validated slice before unchecked decode.
    unsafe { core::str::from_utf8_unchecked(bytes) }
}

#[cfg(test)]
mod tests {
    use super::decode_shadowed;

    #[test]
    fn mentions_decode_shadowed() {
        let _ = stringify!(decode_shadowed);
    }
}
