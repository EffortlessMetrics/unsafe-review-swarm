pub struct Header(u32);

pub fn write_header(bytes: &mut [u8], header: Header) -> Option<()> {
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    let ptr = bytes.as_mut_ptr();
    if !ptr.cast::<Header>().is_aligned() {
        return None;
    }
    // SAFETY: length and alignment are checked above; this fixture still leaves
    // broader pointer validity, allocation, and witness evidence to unsafe-review.
    unsafe { ptr.cast::<Header>().write(header) };
    Some(())
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
