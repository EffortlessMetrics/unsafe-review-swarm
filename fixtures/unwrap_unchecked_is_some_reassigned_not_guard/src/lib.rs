pub fn extract(mut option: Option<u8>) -> u8 {
    if option.is_some() {
        // SAFETY: fixture deliberately invalidates the checked receiver before unwrap.
        option = None;
        unsafe { option.unwrap_unchecked() }
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::extract;

    #[test]
    fn mentions_extract() {
        let _ = stringify!(extract);
    }
}
