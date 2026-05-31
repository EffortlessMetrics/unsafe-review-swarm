pub struct Buffers<'a> {
    pub src: &'a [u8],
    pub dst: &'a mut [u8],
}

pub fn copy_checked<'a>(buffers: &mut Buffers<'a>, count: usize, fallback: &'a mut [u8]) {
    if buffers.src.len() >= count {
        if buffers.dst.len() >= count {
            buffers.dst = fallback;
            // SAFETY: fixture checks that stale open-branch destination path guards are not range evidence.
            unsafe { core::ptr::copy(buffers.src.as_ptr(), buffers.dst.as_mut_ptr(), count) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{copy_checked, Buffers};

    #[test]
    fn copies_into_reassigned_destination() {
        let src = [1_u8, 2, 3, 4];
        let mut dst = [9_u8, 9, 9, 9];
        let mut fallback = [0_u8; 4];
        let mut buffers = Buffers {
            src: &src,
            dst: &mut dst,
        };
        copy_checked(&mut buffers, src.len(), &mut fallback);
        assert_eq!(&*buffers.dst, src.as_slice());
    }
}

