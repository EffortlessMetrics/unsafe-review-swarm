pub fn write_control(register: *mut u32, value: u32) -> Option<()> {
    // SAFETY: this fixture intentionally checks alignment after the volatile write.
    unsafe { register.write_volatile(value) };
    if !register.is_aligned() {
        return None;
    }
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

