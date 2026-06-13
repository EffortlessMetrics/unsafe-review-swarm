/// Allocates a zero-initialised byte buffer of the given length.
/// The body is safe: `vec![0; len]` never invokes `mem::zeroed`.
pub fn zeroed(len: usize) -> Vec<u8> {
    vec![0; len]
}
