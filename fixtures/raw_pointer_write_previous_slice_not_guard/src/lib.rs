use core::mem::MaybeUninit;

pub fn fill_tag(ptr: *mut u16, len: usize, byte: u8) {
    let mut scratch = [MaybeUninit::<u8>::uninit(); 2];
    // SAFETY: this earlier write has same-slice bounds evidence for scratch.
    unsafe { scratch.as_mut_ptr().write_bytes(byte, scratch.len()) }

    let _gap0 = len;
    let _gap1 = byte;
    let _gap2 = ptr;
    let _gap3 = _gap0;
    let _gap4 = _gap1;

    // SAFETY: fixture checks that prior write_bytes evidence does not apply here.
    unsafe { ptr.write_bytes(byte, len) }
}

#[cfg(test)]
mod tests {
    use super::fill_tag;

    #[test]
    fn mentions_fill_tag() {
        let _ = stringify!(fill_tag);
    }
}
