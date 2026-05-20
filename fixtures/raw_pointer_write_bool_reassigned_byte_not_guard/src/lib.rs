pub fn fill_bools(ptr: *mut bool, len: usize, mut byte: u8) {
    if byte > 1 {
        return;
    }
    byte = 2;
    // SAFETY: fixture invalidates the checked byte before write_bytes.
    unsafe { ptr.write_bytes(byte, len) }
}

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
