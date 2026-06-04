pub struct JSValue;
pub struct GlobalObject;
pub struct Callback;
pub struct StringOrBuffer;

pub fn from_js_maybe_async_into<T>(
    _global: &mut GlobalObject,
    _value: JSValue,
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
    pub fn byte_slice(&self) -> &[u8] {
        &[]
    }
}

impl Callback {
    pub fn call(&self, _global: &mut GlobalObject) -> Result<(), ()> {
        Ok(())
    }
}

pub fn async_rab_input(
    global: &mut GlobalObject,
    arg0: JSValue,
    callback: Callback,
) -> Result<usize, ()> {
    callback.call(global)?;
    let input = from_js_maybe_async_into::<StringOrBuffer>(global, arg0)?;
    finish_async_input(&input)
}

fn finish_async_input(input: &StringOrBuffer) -> Result<usize, ()> {
    let bytes = input.byte_slice();
    Ok(bytes.len())
}
