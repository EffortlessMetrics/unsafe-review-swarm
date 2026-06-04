pub struct JSValue;

impl JSValue {
    pub fn to_int32(&self) -> i32 {
        0
    }
}

pub fn read_at(arguments: &[JSValue]) -> Result<usize, ()> {
    let offset = arguments[0].to_int32();
    if offset < 0 {
        record_negative(offset);
    }
    let offset = usize::try_from(offset).expect("offset");
    Ok(offset)
}

fn record_negative(_offset: i32) {}
