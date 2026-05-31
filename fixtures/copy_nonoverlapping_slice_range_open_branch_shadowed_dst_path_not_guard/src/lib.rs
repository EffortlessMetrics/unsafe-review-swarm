pub struct Buffers<'a> {
    pub src: &'a [u8],
    pub dst: &'a mut [u8],
}

pub fn copy_checked<'a>(
    buffers: Buffers<'a>,
    count: usize,
    fallback: &'a mut [u8],
) -> &'a mut [u8] {
    if buffers.src.len() >= count {
        if buffers.dst.len() >= count {
            let buffers = Buffers {
                src: buffers.src,
                dst: fallback,
            };
            // SAFETY: fixture checks that shadowed open-branch destination path guards are not range evidence.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    buffers.src.as_ptr(),
                    buffers.dst.as_mut_ptr(),
                    count,
                )
            }
            return buffers.dst;
        }
    }
    buffers.dst
}

#[cfg(test)]
mod tests {
    use super::{copy_checked, Buffers};

    #[test]
    fn copies_bytes() {
        let src = [1_u8, 2, 3, 4];
        let mut dst = [9_u8, 9, 9, 9];
        let mut fallback = [0_u8; 4];
        let buffers = Buffers {
            src: &src,
            dst: &mut dst,
        };
        let dst = copy_checked(buffers, src.len(), &mut fallback);
        assert_eq!(dst, src.as_slice());
    }
}
