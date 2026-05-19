pub struct Header(u32);

pub fn read_unaligned_header(bytes: &[u8]) -> Option<Header> {
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    let ptr = bytes.as_ptr().cast::<Header>();
    // SAFETY: this fixture checks nullability after the unsafe read.
    let header = unsafe { ptr.read_unaligned() };
    if ptr.is_null() {
        return None;
    }
    Some(header)
}

#[cfg(test)]
mod tests {
    use super::read_unaligned_header;

    #[test]
    fn mentions_read_unaligned_header() {
        let _ = stringify!(read_unaligned_header);
    }
}
