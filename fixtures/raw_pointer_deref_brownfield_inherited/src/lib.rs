/// Legacy header type retained from an early unsafe API layer.
#[derive(Clone, Copy)]
pub struct Header(pub u32);

/// Read a header from a raw pointer.
///
/// Caller must guarantee the pointer is valid and properly aligned.
pub fn read_header(ptr: *const Header) -> Header {
    // Pre-existing debt: no alignment or validity guard visible here.
    // Captured in baseline as inherited debt; the PR does not change this.
    unsafe { *ptr }
}

/// Module-level description added by this PR (safe code, no new unsafe).
pub fn module_name() -> &'static str {
    "brownfield-legacy-io"
}

#[cfg(test)]
mod tests {
    use super::{Header, module_name, read_header};

    #[test]
    fn reads_header() {
        let header = Header(42);
        let _read = read_header(&header);
    }

    #[test]
    fn module_name_is_set() {
        assert_eq!(module_name(), "brownfield-legacy-io");
    }
}
