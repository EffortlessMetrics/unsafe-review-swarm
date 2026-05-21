pub fn reset_then_extend(mut values: Vec<u8>, new_len: usize) -> Vec<u8> {
    assert!(new_len <= values.capacity());
    values = Vec::new();
    // SAFETY: this fixture reassigns the vector after checking capacity and omits initialization.
    unsafe { values.set_len(new_len) }
    values
}

#[cfg(test)]
mod tests {
    use super::reset_then_extend;

    #[test]
    fn mentions_reset_then_extend() {
        let _ = stringify!(reset_then_extend);
    }
}
