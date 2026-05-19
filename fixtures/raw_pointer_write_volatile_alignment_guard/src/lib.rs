pub fn write_control(register: *mut u32, value: u32) -> Option<()> {
    if !register.is_aligned() {
        return None;
    }
    // SAFETY: alignment is checked above; this fixture intentionally omits
    // local nullability, range, and witness evidence for the MMIO register.
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

