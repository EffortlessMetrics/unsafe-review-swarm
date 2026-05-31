pub struct Buffers<'a> {
    pub src: &'a [u8],
    pub dst: &'a mut [u8],
}

pub fn copy_checked<'a>(buffers: Buffers<'a>, count: usize, fallback: &'a mut [u8]) {
    if count > buffers.src.len() || count > buffers.dst.len() {
        return;
    }
    let buffers = Buffers {
        src: buffers.src,
        dst: fallback,
    };
    // SAFETY: fixture checks that stale disjunctive early-return destination
    // path guards are not range evidence after the receiver path is shadowed.
    unsafe {
        core::ptr::copy_nonoverlapping(buffers.src.as_ptr(), buffers.dst.as_mut_ptr(), count)
    }
}

#[cfg(test)]
mod tests {
    use super::{copy_checked, Buffers};

    #[test]
    fn copies_into_shadowed_destination_path() {
        let src = [1_u8, 2, 3, 4];
        let mut dst = [0_u8; 4];
        let mut fallback = [0_u8; 4];
        let buffers = Buffers {
            src: &src,
            dst: &mut dst,
        };
        copy_checked(buffers, src.len(), &mut fallback);
        assert_eq!(dst, [0_u8; 4]);
        assert_eq!(fallback, src);
    }
}
