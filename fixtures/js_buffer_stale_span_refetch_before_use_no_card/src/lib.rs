pub struct JSValue;
pub struct GlobalObject;
pub struct Options;
pub struct ArrayBuffer;

impl ArrayBuffer {
    pub fn as_array_buffer(_global: &mut GlobalObject, _value: JSValue) -> Result<Self, ()> {
        Ok(Self)
    }

    pub fn byte_slice(&self) -> &[u8] {
        &[]
    }
}

impl Options {
    pub fn get(&self, _global: &mut GlobalObject, _name: &str) -> Result<i32, ()> {
        Ok(1)
    }
}

pub fn refetch_then_use(
    global: &mut GlobalObject,
    arg0: JSValue,
    arg1: JSValue,
    options: Options,
) -> Result<usize, ()> {
    let buffer = ArrayBuffer::as_array_buffer(global, arg0)?;
    let bytes = buffer.byte_slice();
    let _hint = bytes.len();
    let offset = options.get(global, "offset")?;
    let fresh = ArrayBuffer::as_array_buffer(global, arg1)?;
    let stable = fresh.byte_slice();
    Ok(stable[offset as usize] as usize)
}
