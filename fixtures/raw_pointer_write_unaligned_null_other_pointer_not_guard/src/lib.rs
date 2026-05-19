pub struct Header(u32);

pub fn write_unaligned_header(bytes: &mut [u8], header: Header, other: *const Header) {
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    let ptr = bytes.as_mut_ptr().cast::<Header>();
    if other.is_null() {
        return;
    }
    // SAFETY: this fixture checks a different pointer, not `ptr`.
    unsafe { ptr.write_unaligned(header) }
}

#[cfg(test)]
mod tests {
    use super::{write_unaligned_header, Header};

    #[test]
    fn mentions_write_unaligned_header() {
        let _ = stringify!(write_unaligned_header);
        let _ = Header(0);
    }
}
