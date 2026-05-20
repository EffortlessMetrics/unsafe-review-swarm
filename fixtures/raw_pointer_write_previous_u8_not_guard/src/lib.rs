pub fn fill_word(ptr: *mut u16, bytes: *mut u8, len: usize, byte: u8) {
    // SAFETY: this earlier byte fill targets a different pointer.
    unsafe { bytes.write_bytes(byte, len) }

    let _gap0 = ptr;
    let _gap1 = byte;
    let _gap2 = len;
    let _gap3 = _gap0;
    let _gap4 = _gap1;
    let _gap5 = _gap2;

    // SAFETY: fixture checks that prior u8 write_bytes evidence does not apply here.
    unsafe { ptr.write_bytes(byte, len) }
}

#[cfg(test)]
mod tests {
    use super::fill_word;

    #[test]
    fn mentions_fill_word() {
        let _ = stringify!(fill_word);
    }
}
