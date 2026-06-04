pub struct JSValue;

impl JSValue {
    pub fn to_int32(&self) -> i32 {
        0
    }
}

pub fn resize(arguments: &[JSValue]) -> Result<usize, ()> {
    let new_len = arguments[0].to_int32();
    let new_len = usize::try_from(new_len).unwrap();
    Ok(new_len)
}
