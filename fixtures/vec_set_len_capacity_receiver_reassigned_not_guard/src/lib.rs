pub fn extend_len(mut values: Vec<u8>, new_len: usize) -> Vec<u8> {
    assert!(new_len <= values.capacity());
    values = Vec::new();
    // SAFETY: this fixture invalidates the checked vector before set_len.
    unsafe { values.set_len(new_len) };
    values
}

#[cfg(test)]
mod tests {
    use super::extend_len;

    #[test]
    fn mentions_extend_len() {
        let _ = stringify!(extend_len);
    }
}
