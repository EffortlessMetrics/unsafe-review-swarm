pub fn copy_byte_to_bool_compound_reassigned(input: u8) -> bool {
    assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());
    let mut value = input;
    assert!(value <= 1);
    value += 1;
    // SAFETY: fixture deliberately invalidates the checked byte before transmute_copy.
    unsafe { core::mem::transmute_copy::<u8, bool>(&value) }
}

#[cfg(test)]
mod tests {
    use super::copy_byte_to_bool_compound_reassigned;

    #[test]
    fn mentions_copy_byte_to_bool_compound_reassigned() {
        let _ = stringify!(copy_byte_to_bool_compound_reassigned);
    }
}

