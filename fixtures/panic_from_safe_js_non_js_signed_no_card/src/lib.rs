pub fn read_at(offset: i32) -> Result<usize, ()> {
    let offset = usize::try_from(offset).expect("offset");
    Ok(offset)
}
