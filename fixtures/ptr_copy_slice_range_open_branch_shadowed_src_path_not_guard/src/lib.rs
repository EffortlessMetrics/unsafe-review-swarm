pub struct Buffers<'a> {
    pub src: &'a [u8],
    pub dst: &'a mut [u8],
}

pub fn copy_checked<'a>(
    buffers: Buffers<'a>,
    count: usize,
    fallback: &'a [u8],
) -> &'a mut [u8] {
    if buffers.src.len() >= count {
        if buffers.dst.len() >= count {
            let buffers = Buffers {
                src: fallback,
                dst: buffers.dst,
            };
            // SAFETY: fixture checks that shadowed open-branch source path guards are not range evidence.
            unsafe { core::ptr::copy(buffers.src.as_ptr(), buffers.dst.as_mut_ptr(), count) }
            return buffers.dst;
        }
    }
    buffers.dst
}

#[cfg(test)]
mod tests {
    use super::{copy_checked, Buffers};

    #[test]
    fn copies_from_shadowed_source() {
        let src = [9_u8, 9, 9, 9];
        let fallback = [1_u8, 2, 3, 4];
        let mut dst = [0_u8; 4];
        let buffers = Buffers {
            src: &src,
            dst: &mut dst,
        };
        let dst = copy_checked(buffers, fallback.len(), &fallback);
        assert_eq!(dst, fallback.as_slice());
    }
}
