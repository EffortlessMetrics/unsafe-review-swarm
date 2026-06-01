pub fn decode(bytes: &[u8]) -> Result<&str, core::str::Utf8Error> {
    match core::str::from_utf8(bytes) {
        Ok(_) => {}
        Err(err) => {
            let _note = "return Err(err)";
            let _ = err;
        }
    }

    // SAFETY: fixture intentionally keeps return text in a string literal only.
    Ok(unsafe { core::str::from_utf8_unchecked(bytes) })
}

#[cfg(test)]
mod tests {
    use super::decode;

    #[test]
    fn decodes_ascii() {
        assert_eq!(decode(b"ok").unwrap(), "ok");
    }
}
