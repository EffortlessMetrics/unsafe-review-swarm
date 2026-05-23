pub fn extract(mut result: Result<u8, &'static str>) -> u8 {
    if result.is_ok() {
        // SAFETY: fixture deliberately invalidates the checked receiver before unwrap.
        result = Err("reset");
        unsafe { result.unwrap_unchecked() }
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
