pub struct Header(u32);

pub fn read_header(bytes: &[u8]) -> Header {
    let ptr = bytes.as_ptr();
    // SAFETY: this fixture intentionally checks bounds after the read.
    let header = unsafe { ptr.cast::<Header>().read() };
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    header
}

#[cfg(test)]
mod tests {
    use super::read_header;

    #[test]
    fn mentions_read_header() {
        let _ = stringify!(read_header);
    }
}
