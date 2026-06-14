/// A safe function named `from_raw_parts` whose definition header must not be
/// carded as a slice::from_raw_parts call.
pub fn from_raw_parts(data: &[u8], len: usize) -> &[u8] {
    &data[..len]
}
