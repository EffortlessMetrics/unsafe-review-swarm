pub struct Header(u32);

pub fn read_header(bytes: &[u8]) -> Option<Header> {
    assert!(bytes.len() >= core::mem::size_of::<Header>());
    let ptr = bytes.as_ptr();
    let null = ptr.is_null();
    observe(null);
    if !ptr.cast::<Header>().is_aligned() {
        return None;
    }
    // SAFETY: this fixture intentionally observes nullability without enforcing it.
    Some(unsafe { ptr.cast::<Header>().read() })
}

fn observe(_null: bool) {}

#[cfg(test)]
mod tests {
    use super::read_header;

    #[test]
    fn mentions_read_header() {
        let _ = stringify!(read_header);
    }
}
