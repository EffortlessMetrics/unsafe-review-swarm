/// Sensor configuration record.
#[derive(Clone, Copy)]
pub struct Config(pub u32);

/// Read the config from a raw pointer.
///
/// # Safety
///
/// The caller must ensure `ptr` is non-null, properly aligned for
/// `Config`, and points to an initialized `Config` value that remains
/// live for the duration of this call.
///
/// This diff adds the safety contract above.  The `pub unsafe fn` and the
/// `*ptr` expression inside are unchanged: the coverage gap resolves
/// because the caller obligations are now documented, not because the
/// unsafe code was removed.
pub unsafe fn read_config(ptr: *const Config) -> Config {
    *ptr
}

#[cfg(test)]
mod tests {
    use super::{Config, read_config};

    #[test]
    fn reads_config() {
        let cfg = Config(7);
        // SAFETY: `cfg` is a valid, aligned, initialized Config on the stack.
        let result = unsafe { read_config(&cfg) };
        assert_eq!(result.0, 7);
    }
}
