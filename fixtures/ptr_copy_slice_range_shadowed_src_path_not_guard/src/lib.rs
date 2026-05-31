pub struct Buffers<'a> {
    pub src: &'a [u8],
    pub dst: &'a mut [u8],
}

pub fn copy_checked<'a>(
    buffers: Buffers<'a>,
    count: usize,
    fallback: &'a [u8],
) -> &'a mut [u8] {
    assert!(buffers.src.len() >= count);
    assert!(buffers.dst.len() >= count);
    let buffers = Buffers {
        src: fallback,
        dst: buffers.dst,
    };
    // SAFETY: fixture checks that a shadowed source path length guard is not range evidence.
    unsafe { core::ptr::copy(buffers.src.as_ptr(), buffers.dst.as_mut_ptr(), count) }
    buffers.dst
}

#[cfg(test)]
mod tests {
    use super::{Buffers, copy_checked};

    #[test]
    fn copies_bytes() {
        let original = [9_u8, 9, 9, 9];
        let fallback = [1_u8, 2, 3, 4];
        let mut dst = [0_u8; 4];
        let buffers = Buffers {
            src: &original,
            dst: &mut dst,
        };
        let dst = copy_checked(buffers, fallback.len(), &fallback);
        assert_eq!(dst, fallback.as_slice());
    }
}
