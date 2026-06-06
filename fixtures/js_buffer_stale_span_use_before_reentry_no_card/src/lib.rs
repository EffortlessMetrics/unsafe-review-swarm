pub struct JSValue;
pub struct GlobalObject;
pub struct Options;
pub struct ArrayBuffer;

impl ArrayBuffer {
    pub fn as_array_buffer(_global: &mut GlobalObject, _value: JSValue) -> Result<Self, ()> {
        Ok(Self)
    }

    pub fn byte_slice(&self) -> &[u8] {
        &[0]
    }
}

impl Options {
    pub fn get(&self, _global: &mut GlobalObject, _name: &str) -> Result<i32, ()> {
        Ok(1)
    }
}

pub fn use_before_reentry(
    global: &mut GlobalObject,
    arg0: JSValue,
    options: Options,
) -> Result<usize, ()> {
    let buffer = ArrayBuffer::as_array_buffer(global, arg0)?;
    let bytes = buffer.byte_slice();
    let first = bytes[0];
    let offset = options.get(global, "offset")?;
    Ok(first as usize + offset as usize)
}
