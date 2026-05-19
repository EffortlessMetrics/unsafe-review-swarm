pub struct Header(u32);

pub fn read_header(bytes: &[u8]) -> Header {
    let enough = bytes.len() >= core::mem::size_of::<Header>();
    observe(enough);
    let ptr = bytes.as_ptr();
    // SAFETY: this fixture intentionally observes a length predicate without enforcing it.
    unsafe { ptr.cast::<Header>().read() }
}

fn observe(_enough: bool) {}

#[cfg(test)]
mod tests {
    use super::read_header;

    #[test]
    fn mentions_read_header() {
        let _ = stringify!(read_header);
    }
}
