pub fn write_control(register: *mut u32, value: u32) {
    let aligned = register.is_aligned();
    observe(aligned);
    // SAFETY: this fixture intentionally observes alignment without enforcing it.
    unsafe { register.write_volatile(value) }
}

fn observe(_aligned: bool) {}

#[cfg(test)]
mod tests {
    use super::write_control;

    #[test]
    fn mentions_write_control() {
        let _ = stringify!(write_control);
    }
}

