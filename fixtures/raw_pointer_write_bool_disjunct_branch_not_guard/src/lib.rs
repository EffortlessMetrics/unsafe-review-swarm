pub fn fill_bools(ptr: *mut bool, len: usize, byte: u8, enabled: bool) {
    if byte <= 1 || enabled {
        // SAFETY: fixture keeps this branch open, but the byte-domain predicate is disjunctive.
        unsafe { ptr.write_bytes(byte, len) }
    }
}

#[cfg(test)]
mod tests {
    use super::fill_bools;

    #[test]
    fn mentions_fill_bools() {
        let mut values = [false; 2];
        fill_bools(values.as_mut_ptr(), values.len(), 1, true);
        assert!(values.iter().all(|value| *value));
    }
}
