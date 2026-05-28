pub fn copy_byte_to_bool_commented(value: u8) -> bool {
    assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());
    // SAFETY: value is a valid bool byte.
    unsafe { core::mem::transmute_copy::<u8, bool>(&value) }
}

#[cfg(test)]
mod tests {
    use super::copy_byte_to_bool_commented;

    #[test]
    fn mentions_copy_byte_to_bool_commented() {
        let _ = stringify!(copy_byte_to_bool_commented);
    }
}

