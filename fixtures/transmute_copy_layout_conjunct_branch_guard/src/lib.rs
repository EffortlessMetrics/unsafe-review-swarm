pub fn copy_byte_to_bool_conjunct_layout(value: u8, allow_conversion: bool) -> bool {
    if core::mem::size_of::<u8>() == core::mem::size_of::<bool>() && allow_conversion {
        // SAFETY: the open branch checks layout, but this fixture intentionally lacks a valid-value proof that value is 0 or 1.
        unsafe { core::mem::transmute_copy::<u8, bool>(&value) }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::copy_byte_to_bool_conjunct_layout;

    #[test]
    fn copies_known_bool_byte() {
        let _value = copy_byte_to_bool_conjunct_layout(1, true);
    }
}

