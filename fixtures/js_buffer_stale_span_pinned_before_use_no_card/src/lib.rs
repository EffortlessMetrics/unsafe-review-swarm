pub struct JSValue;
pub struct GlobalObject;
pub struct Options;
pub struct ArrayBuffer;
pub struct PinnedBuffer;

impl ArrayBuffer {
    pub fn as_array_buffer(_global: &mut GlobalObject, _value: JSValue) -> Result<Self, ()> {
        Ok(Self)
    }

    pub fn byte_slice(&self) -> &[u8] {
        &[]
    }

    pub fn as_pinned_arraybuffer(&self) -> PinnedBuffer {
        PinnedBuffer
    }
}

impl PinnedBuffer {
    pub fn byte_slice(&self) -> &[u8] {
        &[]
    }
}

impl Options {
    pub fn get(&self, _global: &mut GlobalObject, _name: &str) -> Result<i32, ()> {
        Ok(1)
    }
}

pub fn pinned_then_use(
    global: &mut GlobalObject,
    arg0: JSValue,
    options: Options,
) -> Result<usize, ()> {
    let buffer = ArrayBuffer::as_array_buffer(global, arg0)?;
    let bytes = buffer.byte_slice();
    let offset = options.get(global, "offset")?;
    let pinned = buffer.as_pinned_arraybuffer();
    let stable = pinned.byte_slice();
    Ok(stable[offset as usize] as usize + bytes[0] as usize)
}
