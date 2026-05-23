pub fn extract(mut option: Option<u8>) -> u8 {
    let Some(_) = option.as_ref() else {
        return 0;
    };

    // SAFETY: fixture deliberately invalidates the checked receiver before unwrap.
    option = None;
    unsafe { option.unwrap_unchecked() }
}

#[cfg(test)]
mod tests {
    use super::extract;

    #[test]
    fn extracts_some_value() {
        let _ = stringify!(extract);
    }
}
