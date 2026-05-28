pub fn byte_to_bool_disjunct_return(value: u8, disabled: bool) -> bool {
    assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());
    if value > 1 || disabled {
        return false;
    }
    // SAFETY: size equality is asserted and invalid bytes return before the transmute.
    unsafe { core::mem::transmute::<u8, bool>(value) }
}

#[cfg(test)]
mod tests {
    use super::byte_to_bool_disjunct_return;

    #[test]
    fn converts_known_bool_byte() {
        let _value = byte_to_bool_disjunct_return(1, false);
    }
}

