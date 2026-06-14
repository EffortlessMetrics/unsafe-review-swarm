pub struct Header(u32);

pub fn write_header(ptr: *mut Header, other: &[u8], value: Header) {
    if other.len() < core::mem::size_of::<Header>() {
        return;
    }
    // SAFETY: fixture checks that an unrelated slice length guard is not bounds
    // evidence for a raw pointer write to a different destination.
    unsafe { ptr.write(value) }
}

#[cfg(test)]
mod tests {
    use super::write_header;

    #[test]
    fn mentions_write_header() {
        let _ = stringify!(write_header);
    }
}
