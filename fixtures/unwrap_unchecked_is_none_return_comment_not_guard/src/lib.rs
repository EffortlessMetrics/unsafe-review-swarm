pub fn extract(option: Option<u8>) -> u8 {
    if option.is_none() {
        /* return 0; */
    }

    // SAFETY: this fixture intentionally mentions return without actually returning.
    unsafe { option.unwrap_unchecked() }
}

#[cfg(test)]
mod tests {
    use super::extract;

    #[test]
    fn mentions_extract() {
        let _ = stringify!(extract);
    }
}
