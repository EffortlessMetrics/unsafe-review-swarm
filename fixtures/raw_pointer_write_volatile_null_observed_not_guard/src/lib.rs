fn observe(_: bool) {}

pub fn write_control(register: *mut u32, value: u32) {
    let null = register.is_null();
    observe(null);
    // SAFETY: this fixture observes nullability but does not guard on it.
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
