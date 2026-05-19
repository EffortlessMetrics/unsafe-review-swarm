pub fn write_control(register: *mut u32, other: *const u32, value: u32) -> Option<()> {
    if !other.is_aligned() {
        return None;
    }
    // SAFETY: this fixture intentionally checks a different pointer.
    unsafe { register.write_volatile(value) };
    Some(())
}

#[cfg(test)]
mod tests {
    use super::write_control;

    #[test]
    fn mentions_write_control() {
        let _ = stringify!(write_control);
    }
}

