/// A registry type whose path alias ends in `ptr`.
/// The call `registry_ptr::read_entry(i)` must NOT card as a raw-pointer read
/// because `registry_ptr` is an alias path segment, not the `std::ptr` module.
mod registry_ptr {
    pub fn read_entry(index: usize) -> u32 {
        index as u32
    }
}

pub fn fetch(index: usize) -> u32 {
    // This is a safe call — no raw pointer read is involved.
    registry_ptr::read_entry(index)
}

#[cfg(test)]
mod tests {
    use super::fetch;

    #[test]
    fn mentions_fetch() {
        let _ = fetch(0);
    }
}
