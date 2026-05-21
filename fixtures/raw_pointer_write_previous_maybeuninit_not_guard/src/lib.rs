use core::mem::MaybeUninit;

pub fn fill_word(ptr: *mut u16, scratch: &mut [MaybeUninit<u8>], len: usize, byte: u8) {
    // SAFETY: this earlier write targets MaybeUninit storage, not `ptr`.
    unsafe { scratch.as_mut_ptr().write_bytes(byte, scratch.len()) }

    let _gap0 = ptr;
    let _gap1 = len;
    let _gap2 = byte;
    let _gap3 = _gap0;

    // SAFETY: fixture checks that prior MaybeUninit write evidence does not apply here.
    unsafe { ptr.write_bytes(byte, len) }
}

#[cfg(test)]
mod tests {
    use super::fill_word;
    use core::mem::MaybeUninit;

    #[test]
    fn mentions_fill_word() {
        let mut scratch = [MaybeUninit::<u8>::uninit(); 2];
        let _ = scratch.as_mut_ptr();
        let _ = stringify!(fill_word);
    }
}
