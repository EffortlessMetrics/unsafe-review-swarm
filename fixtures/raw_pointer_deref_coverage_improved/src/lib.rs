/// A sensor config record.
#[derive(Clone, Copy)]
pub struct Config(pub u32);

/// Read config from a raw pointer.
///
/// # Safety
///
/// The caller must ensure `ptr` is non-null, properly aligned for `Config`,
/// points to an initialized `Config` value that remains live for this call,
/// and that the access stays within one allocation.
///
/// This PR adds the safety contract above; the underlying unsafe dereference
/// is unchanged.  The coverage slot for contract moves from `missing` to
/// `present` — an evidence improvement, NOT a safety proof or resolution.
/// The unsafe site remains open and advisory.
pub fn read_config(ptr: *const Config) -> Config {
    // SAFETY: caller guarantees the invariants documented in the function-level
    // # Safety section.
    unsafe { *ptr }
}

#[cfg(test)]
mod tests {
    use super::{Config, read_config};

    #[test]
    fn reads_config() {
        let cfg = Config(42);
        // SAFETY: `cfg` is valid, aligned, initialized on the stack.
        let result = unsafe { read_config(&cfg) };
        assert_eq!(result.0, 42);
    }
}
