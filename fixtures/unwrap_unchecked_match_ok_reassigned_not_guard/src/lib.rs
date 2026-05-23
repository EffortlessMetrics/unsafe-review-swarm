pub fn extract(mut result: Result<u8, &'static str>) -> u8 {
    match result.as_ref() {
        Ok(_) => {
            // SAFETY: fixture deliberately invalidates the checked receiver before unwrap.
            result = Err("reset");
            unsafe { result.unwrap_unchecked() }
        }
        Err(_) => 0,
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
