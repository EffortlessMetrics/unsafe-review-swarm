pub fn read_status(register: *const u32, other: *const u32) -> Option<u32> {
    if other.is_null() {
        return None;
    }
    // SAFETY: this fixture checks a different pointer, not `register`.
    Some(unsafe { register.read_volatile() })
}

#[cfg(test)]
mod tests {
    use super::read_status;

    #[test]
    fn mentions_read_status() {
        let _ = stringify!(read_status);
    }
}
