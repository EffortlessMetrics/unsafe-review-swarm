pub struct Header(u32);

pub fn read_header(bytes: &[u8], other: &[u8]) -> Header {
    if bytes.len() >= core::mem::size_of::<Header>() {
        let bytes = other;
        let ptr = bytes.as_ptr();
        // SAFETY: fixture checks that an open branch over a shadowed origin is stale.
        let header = unsafe { ptr.cast::<Header>().read() };
        return header;
    }
    Header(0)
}

#[cfg(test)]
mod tests {
    use super::read_header;

    #[test]
    fn mentions_read_header() {
        let _ = stringify!(read_header);
    }
}
