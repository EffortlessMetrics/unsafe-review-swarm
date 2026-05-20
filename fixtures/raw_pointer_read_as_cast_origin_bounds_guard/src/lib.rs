pub struct Header(u32);

pub fn read_header(bytes: &[u8]) -> Header {
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    let ptr = bytes.as_ptr() as *const Header;
    // SAFETY: fixture documents the as-cast pointer's same-origin length guard
    // but intentionally omits the other raw-read proof obligations.
    unsafe { ptr.read() }
}

#[cfg(test)]
mod tests {
    use super::read_header;

    #[test]
    fn mentions_read_header() {
        let _ = stringify!(read_header);
    }
}
