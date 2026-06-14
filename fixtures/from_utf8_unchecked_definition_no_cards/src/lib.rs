/// A safe function named `from_utf8_unchecked` whose definition header must
/// not be carded as a str::from_utf8_unchecked call.
pub fn from_utf8_unchecked(bytes: &[u8]) -> Result<&str, std::str::Utf8Error> {
    std::str::from_utf8(bytes)
}
