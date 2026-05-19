pub fn read_status(register: *const u32) -> Option<u32> {
    if !register.is_aligned() {
        return None;
    }
    // SAFETY: alignment is checked above; this fixture intentionally omits
    // local nullability, range, and witness evidence for the MMIO register.
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

