pub fn extend_len(values: &mut Vec<u8>, mut new_len: usize) {
    assert!(new_len <= values.capacity());
    new_len = values.capacity() + 1;
    // SAFETY: this fixture invalidates the checked length before set_len.
    unsafe { values.set_len(new_len) }
}

#[cfg(test)]
mod tests {
    use super::extend_len;

    #[test]
    fn mentions_extend_len() {
        let mut values = Vec::with_capacity(4);
        let _ = stringify!(extend_len);
        let _ = &mut values;
    }
}
