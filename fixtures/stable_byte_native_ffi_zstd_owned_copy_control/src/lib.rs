unsafe extern "C" {
    fn zstd_compress_into(
        src: *const u8,
        src_len: usize,
        dst: *mut u8,
        dst_len: usize,
    ) -> usize;
}

pub struct JSValue;
pub struct GlobalObject;

pub struct JSArrayBufferView {
    ptr: *const u8,
    len: usize,
}

impl JSArrayBufferView {
    pub fn from_js(_global: &mut GlobalObject, _value: JSValue) -> Self {
        Self {
            ptr: core::ptr::null(),
            len: 0,
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        Vec::new()
    }
}

pub fn zstd_overlap_handoff(
    global: &mut GlobalObject,
    value: JSValue,
    output: &mut [u8],
) -> usize {
    let input = JSArrayBufferView::from_js(global, value);
    let bytes = input.to_vec();
    // SAFETY: this fixture keeps the generic native FFI seam while proving the
    // native-FFI stable-byte heuristic does not fire after an owned copy.
    unsafe { zstd_compress_into(bytes.as_ptr(), bytes.len(), output.as_mut_ptr(), output.len()) }
}

#[cfg(test)]
mod tests {
    use super::{GlobalObject, JSValue, zstd_overlap_handoff};

    #[test]
    fn mentions_zstd_overlap_handoff() {
        let mut global = GlobalObject;
        let mut output = [];
        let _size = zstd_overlap_handoff(&mut global, JSValue, &mut output);
    }
}
