pub struct Buffers<'a> {
    pub src: &'a [u8],
    pub dst: &'a mut [u8],
}

pub fn copy_checked<'a>(buffers: &mut Buffers<'a>, count: usize, fallback: &'a [u8]) {
    assert!(buffers.src.len() >= count);
    assert!(buffers.dst.len() >= count);
    buffers.src = fallback;
    // SAFETY: fixture checks that a stale source path length guard is not range evidence.
    unsafe { core::ptr::copy(buffers.src.as_ptr(), buffers.dst.as_mut_ptr(), count) }
}

#[cfg(test)]
mod tests {
    use super::{Buffers, copy_checked};

    #[test]
    fn copies_bytes() {
        let original = [9_u8, 9, 9, 9];
        let fallback = [1_u8, 2, 3, 4];
        let mut dst = [0_u8; 4];
        let mut buffers = Buffers {
            src: &original,
            dst: &mut dst,
        };
        copy_checked(&mut buffers, fallback.len(), &fallback);
        assert_eq!(&*buffers.dst, fallback.as_slice());
    }
}
