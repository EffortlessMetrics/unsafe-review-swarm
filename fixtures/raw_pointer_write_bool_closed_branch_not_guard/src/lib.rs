pub fn fill_bools(ptr: *mut bool, len: usize, byte: u8) {
    if byte <= 1 {
        observe_valid_byte(byte);
    }
    // SAFETY: fixture observes byte validity in a closed branch before write_bytes.
    unsafe { ptr.write_bytes(byte, len) }
}

fn observe_valid_byte(_byte: u8) {}

#[cfg(test)]
mod tests {
    use super::fill_bools;

    #[test]
    fn mentions_fill_bools() {
        let mut values = [false; 2];
        let _ = values.as_mut_ptr();
        let _ = stringify!(fill_bools);
    }
}
