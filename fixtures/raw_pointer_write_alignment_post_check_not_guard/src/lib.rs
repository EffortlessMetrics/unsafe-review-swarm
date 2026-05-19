pub struct Header(u32);

pub fn write_header(bytes: &mut [u8], header: Header) -> Option<()> {
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    let ptr = bytes.as_mut_ptr();
    // SAFETY: this fixture intentionally checks alignment after the write.
    unsafe { ptr.cast::<Header>().write(header) };
    if !ptr.cast::<Header>().is_aligned() {
        return None;
    }
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
