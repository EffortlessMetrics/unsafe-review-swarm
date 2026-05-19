fn observe(_: bool) {}

pub fn read_status(register: *const u32) -> Option<u32> {
    let null = register.is_null();
    observe(null);
    // SAFETY: this fixture observes nullability but does not guard on it.
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
