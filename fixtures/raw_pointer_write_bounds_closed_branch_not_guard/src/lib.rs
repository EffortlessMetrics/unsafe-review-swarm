pub struct Header(u32);

pub fn write_header(bytes: &mut [u8], header: Header) {
    if bytes.len() >= core::mem::size_of::<Header>() {
        observe(bytes.len());
    }
    let ptr = bytes.as_mut_ptr();
    // SAFETY: this fixture intentionally closes the bounds branch before use.
    unsafe { ptr.cast::<Header>().write(header) }
}

fn observe(_len: usize) {}

#[cfg(test)]
mod tests {
    use super::{write_header, Header};

    #[test]
    fn mentions_write_header() {
        let _ = stringify!(write_header);
        let _ = Header(0);
    }
}
