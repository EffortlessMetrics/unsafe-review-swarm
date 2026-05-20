pub fn fill_bools(ptr: *mut bool, len: usize, byte: u8) {
    if byte > 1 {
        return;
    }
    // SAFETY: fixture keeps pointer/bounds/allocation proof absent but guards bool byte validity.
    unsafe { ptr.write_bytes(byte, len) }
}

#[cfg(test)]
mod tests {
    use super::fill_bools;

    #[test]
    fn mentions_fill_bools() {
        let mut values = [false; 2];
        fill_bools(values.as_mut_ptr(), values.len(), 1);
        assert!(values.iter().all(|value| *value));
    }
}
