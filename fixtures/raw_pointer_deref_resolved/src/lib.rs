/// Sensor configuration record.
#[derive(Clone, Copy)]
pub struct Config(pub u32);

/// Return the config value from a shared reference.
///
/// The PR diff replaced the original raw-pointer deref (which had no
/// safety contract or guard) with a safe reference parameter.  The
/// baseline-captured card for the old `unsafe { *ptr }` site is now
/// absent because the unsafe expression was removed: gap resolved.
pub fn read_config(config: &Config) -> Config {
    *config
}

#[cfg(test)]
mod tests {
    use super::{Config, read_config};

    #[test]
    fn reads_config() {
        let cfg = Config(7);
        assert_eq!(read_config(&cfg).0, 7);
    }
}
