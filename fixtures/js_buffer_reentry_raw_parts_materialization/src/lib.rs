pub struct JSValue;
pub struct GlobalObject;
pub struct Options;

pub struct StringOrBuffer {
    ptr: *const u8,
    len: usize,
}

impl StringOrBuffer {
    pub fn from_js(_global: &mut GlobalObject, _value: JSValue) -> Result<Self, ()> {
        Ok(Self {
            ptr: core::ptr::null(),
            len: 0,
        })
    }
}

impl Options {
    pub fn get(&self, _global: &mut GlobalObject, _name: &str) -> Result<i32, ()> {
        Ok(1)
    }
}

pub fn zstd_raw_parts(
    global: &mut GlobalObject,
    arg0: JSValue,
    options: Options,
) -> Result<usize, ()> {
    let input = StringOrBuffer::from_js(global, arg0)?;
    let level = options.get(global, "level")?;
    let bytes = unsafe { core::slice::from_raw_parts(input.ptr, input.len) };
    Ok(bytes.len() + level as usize)
}
