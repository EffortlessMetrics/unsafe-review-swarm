pub struct JSValue;

impl JSValue {
    pub fn to_int32(&self) -> i32 {
        0
    }
}

pub fn read_at(arguments: &[JSValue]) -> Result<usize, ()> {
    let offset = usize::try_from(arguments[0].to_int32()).expect("offset");
    Ok(offset)
}
