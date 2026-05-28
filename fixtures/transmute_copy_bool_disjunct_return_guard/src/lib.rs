pub fn copy_byte_to_bool_disjunct_return(value: u8, disabled: bool) -> bool {
    assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());
    if value > 1 || disabled {
        return false;
    }
    // SAFETY: size equality is asserted and invalid referenced bytes return before transmute_copy.
    unsafe { core::mem::transmute_copy::<u8, bool>(&value) }
}

#[cfg(test)]
mod tests {
    use super::copy_byte_to_bool_disjunct_return;

    #[test]
    fn copies_known_bool_byte() {
        let _value = copy_byte_to_bool_disjunct_return(1, false);
    }
}

