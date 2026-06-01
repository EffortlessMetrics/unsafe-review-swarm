pub fn decode_shadowed<'a>(bytes: &'a [u8], fallback: &'a [u8]) -> &'a str {
    match core::str::from_utf8(bytes) {
        Ok(_) => {
            let bytes = fallback;
            // SAFETY: fixture deliberately shadows the validated slice before unchecked decode.
            unsafe { core::str::from_utf8_unchecked(bytes) }
        }
        Err(_) => "",
    }
}

#[cfg(test)]
mod tests {
    use super::decode_shadowed;

    #[test]
    fn mentions_decode_shadowed() {
        let _ = stringify!(decode_shadowed);
    }
}
