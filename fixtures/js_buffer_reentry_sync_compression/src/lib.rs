pub struct JSValue;
pub struct GlobalObject;
pub struct Options;
pub struct StringOrBuffer;

impl StringOrBuffer {
    pub fn from_js(_global: &mut GlobalObject, _value: JSValue) -> Result<Self, ()> {
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

pub fn zstd_sync(
    global: &mut GlobalObject,
    arg0: JSValue,
    options: Options,
) -> Result<usize, ()> {
    let input = StringOrBuffer::from_js(global, arg0)?;
    let level = options.get(global, "level")?;
    native_compress(&input, level)
}

fn native_compress(input: &StringOrBuffer, level: i32) -> Result<usize, ()> {
    let bytes = input.byte_slice();
    Ok(bytes.len() + level as usize)
}
