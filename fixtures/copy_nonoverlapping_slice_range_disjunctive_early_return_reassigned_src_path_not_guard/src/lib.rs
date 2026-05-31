pub struct Buffers<'a> {
    pub src: &'a [u8],
    pub dst: &'a mut [u8],
}

pub fn copy_checked<'a>(buffers: &mut Buffers<'a>, count: usize, fallback: &'a [u8]) {
    if count > buffers.src.len() || count > buffers.dst.len() {
        return;
    }
    buffers.src = fallback;
    // SAFETY: fixture checks that stale disjunctive early-return source path guards are not range evidence.
    unsafe {
        core::ptr::copy_nonoverlapping(buffers.src.as_ptr(), buffers.dst.as_mut_ptr(), count)
    }
}
#[cfg(test)]
mod tests {
    use super::{copy_checked, Buffers};

    #[test]
    fn copies_from_reassigned_source() {
        let src = [1_u8, 2, 3, 4];
        let fallback = [5_u8, 6, 7, 8];
        let mut dst = [0_u8; 4];
        let mut buffers = Buffers {
            src: &src,
            dst: &mut dst,
        };
        copy_checked(&mut buffers, fallback.len(), &fallback);
        assert_eq!(buffers.dst, fallback);
    }
}
