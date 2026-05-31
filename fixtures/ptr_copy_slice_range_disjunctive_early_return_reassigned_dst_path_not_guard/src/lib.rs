pub struct Buffers<'a> {
    pub src: &'a [u8],
    pub dst: &'a mut [u8],
}

pub fn copy_overlapping_checked<'a>(
    buffers: &mut Buffers<'a>,
    count: usize,
    fallback: &'a mut [u8],
) {
    if count > buffers.src.len() || count > buffers.dst.len() {
        return;
    }
    buffers.dst = fallback;
    // SAFETY: fixture checks that stale disjunctive early-return destination
    // path guards are not range evidence for overlapping copy.
    unsafe { core::ptr::copy(buffers.src.as_ptr(), buffers.dst.as_mut_ptr(), count) }
}

#[cfg(test)]
mod tests {
    use super::{copy_overlapping_checked, Buffers};

    #[test]
    fn copies_into_reassigned_destination_path() {
        let src = [1_u8, 2, 3, 4];
        let mut dst = [0_u8; 4];
        let mut fallback = [0_u8; 4];
        let mut buffers = Buffers {
            src: &src,
            dst: &mut dst,
        };
        copy_overlapping_checked(&mut buffers, src.len(), &mut fallback);
        assert_eq!(&*buffers.dst, &src);
    }
}
