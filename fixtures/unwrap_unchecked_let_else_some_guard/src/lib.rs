pub fn extract(option: Option<u8>) -> u8 {
    let Some(_) = option.as_ref() else {
        return 0;
    };

    // SAFETY: the let-else above returns before this point when the option is None.
    unsafe { option.unwrap_unchecked() }
}

#[cfg(test)]
mod tests {
    use super::extract;

    #[test]
    fn extracts_some_value() {
        assert_eq!(extract(Some(7)), 7);
        assert_eq!(extract(None), 0);
    }
}
