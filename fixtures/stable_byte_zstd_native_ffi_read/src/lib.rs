pub struct JsBuffer;
pub struct ZstdContext;

impl JsBuffer {
    pub fn byte_slice(&self) -> &[u8] {
        &[]
    }

    pub fn byte_slice_mut(&mut self) -> &mut [u8] {
        &mut []
    }
}

impl ZstdContext {
    pub fn set_buffers(&mut self, _input: Option<&[u8]>, _output: Option<&mut [u8]>) {}

    pub fn compress(&mut self) -> Result<usize, ()> {
        Ok(0)
    }
}

pub fn zstd_overlap_native_ffi(
    input: &JsBuffer,
    output: &mut JsBuffer,
    ctx: &mut ZstdContext,
) -> Result<usize, ()> {
    let input_bytes = input.byte_slice();
    let output_bytes = output.byte_slice_mut();
    ctx.set_buffers(Some(input_bytes), Some(output_bytes));
    ctx.compress()
}
