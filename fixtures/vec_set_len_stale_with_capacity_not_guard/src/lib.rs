pub fn reset_after_capacity(new_len: usize) -> Vec<u8> {
    let mut values = Vec::with_capacity(new_len);
    values = Vec::new();
    // SAFETY: this fixture checks that stale with_capacity evidence does not apply after reassignment.
    unsafe { values.set_len(new_len) };
    values
}

#[cfg(test)]
mod tests {
    use super::reset_after_capacity;

    #[test]
    fn mentions_reset_after_capacity() {
        let _ = stringify!(reset_after_capacity);
    }
}

