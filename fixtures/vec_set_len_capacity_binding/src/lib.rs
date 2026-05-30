pub fn extend_after_capacity_binding(mut values: Vec<u8>, new_len: usize) -> Vec<u8> {
    let capacity = values.capacity();
    assert!(new_len <= capacity);
    // SAFETY: this fixture has capacity evidence through a binding, but it omits initialization.
    unsafe { values.set_len(new_len) };
    values
}

#[cfg(test)]
mod tests {
    use super::extend_after_capacity_binding;

    #[test]
    fn mentions_extend_after_capacity_binding() {
        let _ = stringify!(extend_after_capacity_binding);
    }
}
