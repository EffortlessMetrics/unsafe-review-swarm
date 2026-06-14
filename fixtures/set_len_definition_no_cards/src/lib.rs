/// A safe function named `set_len` whose definition header must not be carded
/// as a Vec::set_len call.
pub fn set_len(v: &mut Vec<u8>, new_len: usize) {
    v.truncate(new_len);
}
