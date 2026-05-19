pub fn write_control(register: *mut u32, value: u32) {
    if register.is_null() {
        return;
    }
    // SAFETY: nullability is checked above; this fixture intentionally omits
    // local alignment, range, and witness evidence for the MMIO register.
    unsafe { register.write_volatile(value) }
}

#[cfg(test)]
mod tests {
    use super::write_control;

    #[test]
    fn mentions_write_control() {
        let _ = stringify!(write_control);
    }
}
