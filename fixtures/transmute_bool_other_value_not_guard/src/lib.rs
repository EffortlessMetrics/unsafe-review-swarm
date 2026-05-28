pub fn byte_to_bool_checked_other(value: u8, other: u8) -> bool {
    assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());
    assert!(other <= 1);
    // SAFETY: this fixture intentionally checks a different byte before transmute.
    unsafe { core::mem::transmute::<u8, bool>(value) }
}

#[cfg(test)]
mod tests {
    use super::byte_to_bool_checked_other;

    #[test]
    fn mentions_byte_to_bool_checked_other() {
        let _ = stringify!(byte_to_bool_checked_other);
    }
}

