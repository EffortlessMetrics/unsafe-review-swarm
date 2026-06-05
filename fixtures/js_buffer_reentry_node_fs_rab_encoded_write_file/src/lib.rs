pub struct JSValue;
pub struct GlobalObject;
pub struct StringOrBuffer;
pub struct Encoding;

pub fn from_js_with_encoding_maybe_async_into<T>(
    _global: &mut GlobalObject,
    _value: JSValue,
    _encoding: Encoding,
) -> Result<T, ()>
where
    T: Default,
{
    Ok(T::default())
}

impl Default for StringOrBuffer {
    fn default() -> Self {
        Self
    }
}

impl StringOrBuffer {
    pub fn slice(&self) -> &[u8] {
        &[]
    }
}

pub struct WriteFileArgs {
    data: StringOrBuffer,
}

pub fn node_fs_rab_encoded_write_file(
    global: &mut GlobalObject,
    data: JSValue,
    encoding: Encoding,
) -> Result<usize, ()> {
    let args = WriteFileArgs {
        data: from_js_with_encoding_maybe_async_into::<StringOrBuffer>(global, data, encoding)?,
    };
    dispatch_async_worker(&args)?;
    write_file_worker(&args)
}

fn dispatch_async_worker(_args: &WriteFileArgs) -> Result<(), ()> {
    Ok(())
}

fn write_file_worker(args: &WriteFileArgs) -> Result<usize, ()> {
    let bytes = args.data.slice();
    Ok(bytes.len())
}
