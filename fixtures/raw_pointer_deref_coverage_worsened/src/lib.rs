/// A sensor config record.
#[derive(Clone, Copy)]
pub struct Config(pub u32);

/// Read config from a raw pointer.
///
/// Caller must ensure the pointer is valid and properly aligned.
pub fn write_config(ptr: *mut Config, value: Config) {
    unsafe { *ptr = value }
}

#[cfg(test)]
mod tests {
    use super::{Config, write_config};

    #[test]
    fn writes_config() {
        let mut cfg = Config(0);
        // SAFETY: `cfg` is valid, aligned, initialized on the stack.
        unsafe { write_config(&mut cfg, Config(42)) };
        assert_eq!(cfg.0, 42);
    }
}
