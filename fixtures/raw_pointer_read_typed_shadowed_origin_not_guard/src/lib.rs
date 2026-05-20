pub struct Header(u32);

pub fn read_header(bytes: &[u8], other: &[u8]) -> Header {
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    let bytes: &[u8] = other;
    let ptr = bytes.as_ptr();
    // SAFETY: fixture checks that typed shadowing makes the earlier bounds assertion stale.
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
