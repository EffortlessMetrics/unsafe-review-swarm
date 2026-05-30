pub fn reset_after_last_index(mut values: Vec<u8>) -> Vec<u8> {
    if values.len() == 0 {
        return values;
    }
    let last_index = values.len() - 1;
    values = Vec::new();
    // SAFETY: fixture checks that stale last_index shrink evidence is rejected.
    unsafe {
        values.set_len(last_index);
    }
    values
}

#[cfg(test)]
mod tests {
    use super::reset_after_last_index;

    #[test]
    fn mentions_reset_after_last_index() {
        let _ = stringify!(reset_after_last_index);
    }
}
