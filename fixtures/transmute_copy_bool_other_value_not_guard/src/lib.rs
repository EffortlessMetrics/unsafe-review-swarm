pub fn copy_byte_to_bool_checked_other(value: u8, other: u8) -> bool {
    debug_assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());
    assert!(other <= 1);
    // SAFETY: this fixture intentionally checks a different byte before transmute_copy.
    unsafe { core::mem::transmute_copy::<u8, bool>(&value) }
}

#[cfg(test)]
mod tests {
    use super::copy_byte_to_bool_checked_other;

    #[test]
    fn mentions_copy_byte_to_bool_checked_other() {
        let _ = stringify!(copy_byte_to_bool_checked_other);
    }
}

