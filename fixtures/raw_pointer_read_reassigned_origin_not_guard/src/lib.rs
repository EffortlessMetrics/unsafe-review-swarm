pub struct Header(u32);

pub fn read_header(bytes: &[u8], other: &[u8]) -> Header {
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    let mut ptr = bytes.as_ptr();
    ptr = other.as_ptr();
    // SAFETY: fixture checks that stale origin guards are not bounds evidence.
    unsafe { ptr.cast::<Header>().read() }
}

#[cfg(test)]
mod tests {
    use super::read_header;

    #[test]
    fn mentions_read_header() {
        let _ = stringify!(read_header);
    }
}
