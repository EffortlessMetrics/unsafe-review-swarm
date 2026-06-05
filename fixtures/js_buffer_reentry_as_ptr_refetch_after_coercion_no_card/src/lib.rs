pub struct JSValue;
pub struct GlobalObject;
pub struct ArrayBuffer {
    bytes: Vec<u8>,
}

impl JSValue {
    pub fn as_array_buffer(&self, _global: &mut GlobalObject) -> Result<ArrayBuffer, ()> {
        Ok(ArrayBuffer { bytes: vec![] })
    }

    pub fn coerce_to_int64(&self, _global: &mut GlobalObject) -> Result<i64, ()> {
        Ok(0)
    }
}

impl ArrayBuffer {
    pub fn as_ptr(&self) -> *const u8 {
        self.bytes.as_ptr()
    }
}

pub fn pointer_refetch_route(
    global: &mut GlobalObject,
    source: JSValue,
    offset: JSValue,
) -> Result<usize, ()> {
    let buffer = source.as_array_buffer(global)?;
    let start = offset.coerce_to_int64(global)?;
    let fresh = source.as_array_buffer(global)?;
    let ptr = fresh.as_ptr();
    Ok(ptr as usize + start as usize + buffer.bytes.len())
}
