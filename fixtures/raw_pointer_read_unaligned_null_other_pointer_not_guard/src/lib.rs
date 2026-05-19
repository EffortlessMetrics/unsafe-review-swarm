pub struct Header(u32);

pub fn read_unaligned_header(bytes: &[u8], other: *const Header) -> Option<Header> {
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    let ptr = bytes.as_ptr().cast::<Header>();
    if other.is_null() {
        return None;
    }
    // SAFETY: this fixture checks a different pointer, not `ptr`.
    Some(unsafe { ptr.read_unaligned() })
}

#[cfg(test)]
mod tests {
    use super::read_unaligned_header;

    #[test]
    fn mentions_read_unaligned_header() {
        let _ = stringify!(read_unaligned_header);
    }
}
