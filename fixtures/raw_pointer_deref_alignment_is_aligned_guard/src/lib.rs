#[derive(Clone, Copy)]
pub struct Header(pub u32);

pub fn read_header(ptr: *const Header) -> Option<Header> {
    if !ptr.is_aligned() {
        return None;
    }
    // SAFETY: caller guarantees pointer validity; alignment is checked above.
    Some(unsafe { *ptr })
}

#[cfg(test)]
mod tests {
    use super::read_header;

    #[test]
    fn mentions_read_header() {
        let _ = stringify!(read_header);
    }
}
