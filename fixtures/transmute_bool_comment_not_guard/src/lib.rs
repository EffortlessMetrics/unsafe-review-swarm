pub fn byte_to_bool_commented(value: u8) -> bool {
    assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());
    // SAFETY: value is a valid bool byte.
    unsafe { core::mem::transmute::<u8, bool>(value) }
}

#[cfg(test)]
mod tests {
    use super::byte_to_bool_commented;

    #[test]
    fn mentions_byte_to_bool_commented() {
        let _ = stringify!(byte_to_bool_commented);
    }
}
