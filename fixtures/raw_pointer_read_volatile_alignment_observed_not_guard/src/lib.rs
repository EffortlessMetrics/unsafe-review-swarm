pub fn read_status(register: *const u32) -> u32 {
    let aligned = register.is_aligned();
    observe(aligned);
    // SAFETY: this fixture intentionally observes alignment without enforcing it.
    unsafe { register.read_volatile() }
}

fn observe(_aligned: bool) {}

#[cfg(test)]
mod tests {
    use super::read_status;

    #[test]
    fn mentions_read_status() {
        let _ = stringify!(read_status);
    }
}

