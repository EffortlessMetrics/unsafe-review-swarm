pub struct Header(u32);

pub fn write_header(bytes: &mut [u8], other: *mut u8, header: Header) {
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    let ptr = bytes.as_mut_ptr();
    if other.is_null() {
        return;
    }
    if !ptr.cast::<Header>().is_aligned() {
        return;
    }
    // SAFETY: this fixture intentionally checks a different pointer for null.
    unsafe { ptr.cast::<Header>().write(header) }
}

#[cfg(test)]
mod tests {
    use super::{write_header, Header};

    #[test]
    fn mentions_write_header() {
        let _ = stringify!(write_header);
        let _ = Header(0);
    }
}
