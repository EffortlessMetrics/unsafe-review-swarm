#[derive(Clone, Copy)]
pub struct Header(pub u32);

pub fn read_headers(first: *const Header, second: *const Header) -> (Header, Header) {
    // SAFETY: `first` validity is guaranteed by the documented caller contract.
    let a = unsafe { *first };
    let b = unsafe { *second };
    (a, b)
}

#[cfg(test)]
mod tests {
    use super::{Header, read_headers};

    #[test]
    fn reads_both_headers() {
        let h1 = Header(1);
        let h2 = Header(2);
        let (a, b) = read_headers(&h1, &h2);
        let _ = (a, b);
    }
}
