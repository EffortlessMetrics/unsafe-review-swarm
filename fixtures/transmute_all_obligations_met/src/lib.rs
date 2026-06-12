/// Probe: a transmute that provides both layout evidence (size_of equality
/// assertion) AND valid-value evidence (assert that the value is in-range).
/// The tool should classify this as guarded_unwitnessed — the guard is present
/// for both transmute obligations but no Miri witness receipt is attached.
pub fn byte_to_bool_both_guards(value: u8) -> bool {
    assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());
    assert!(value <= 1, "value must be a valid bool byte");
    // SAFETY: size equality is checked; value is 0 or 1 (valid bool bytes).
    unsafe { core::mem::transmute::<u8, bool>(value) }
}

#[cfg(test)]
mod tests {
    use super::byte_to_bool_both_guards;

    #[test]
    fn probe_both_guards_true() {
        assert!(byte_to_bool_both_guards(1));
    }

    #[test]
    fn probe_both_guards_false() {
        assert!(!byte_to_bool_both_guards(0));
    }
}
