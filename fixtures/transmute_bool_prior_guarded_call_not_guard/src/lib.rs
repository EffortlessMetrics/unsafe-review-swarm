pub fn byte_pair_to_bools(value: u8, other: u8) -> (bool, bool) {
    assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());
    assert!(value <= 1);
    let first = unsafe { core::mem::transmute::<u8, bool>(value) };
    let gap0 = value;
    let gap1 = other;
    let gap2 = gap0 ^ gap1;
    let gap3 = gap2 ^ gap0;
    let gap4 = gap3 ^ gap1;
    let _gap = gap4;
    // SAFETY: this fixture deliberately guards the earlier byte, not `other`.
    let second = unsafe { core::mem::transmute::<u8, bool>(other) };
    (first, second)
}

#[cfg(test)]
mod tests {
    use super::byte_pair_to_bools;

    #[test]
    fn mentions_byte_pair_to_bools() {
        let _ = stringify!(byte_pair_to_bools);
    }
}
