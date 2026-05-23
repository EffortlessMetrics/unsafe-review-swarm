pub fn extract(mut option: Option<u8>) -> u8 {
    match option.as_ref() {
        Some(_) => {
            // SAFETY: fixture deliberately invalidates the checked receiver before unwrap.
            option = None;
            unsafe { option.unwrap_unchecked() }
        }
        None => 0,
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
