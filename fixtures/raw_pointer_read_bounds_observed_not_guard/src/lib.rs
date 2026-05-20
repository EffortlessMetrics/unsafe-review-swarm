pub struct Header(u32);

pub fn read_header(bytes: &[u8]) -> Header {
    if bytes.len() >= core::mem::size_of::<Header>() {
        observe(bytes.len());
    }
    let ptr = bytes.as_ptr();
    // SAFETY: fixture checks that a closed observed bounds branch is not evidence.
    unsafe { ptr.cast::<Header>().read() }
}

fn observe(_len: usize) {}

#[cfg(test)]
mod tests {
    use super::read_header;

    #[test]
    fn mentions_read_header() {
        let _ = stringify!(read_header);
    }
}
