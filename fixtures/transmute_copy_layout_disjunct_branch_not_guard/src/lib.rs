pub fn copy_byte_to_bool_disjunct_layout(value: u8, allow_conversion: bool) -> bool {
    if core::mem::size_of::<u8>() == core::mem::size_of::<bool>() || allow_conversion {
        // SAFETY: a disjunctive layout branch can enter without the size check being true, and this fixture lacks bool-domain evidence.
        unsafe { core::mem::transmute_copy::<u8, bool>(&value) }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::copy_byte_to_bool_disjunct_layout;

    #[test]
    fn mentions_copy_byte_to_bool_disjunct_layout() {
        let _ = stringify!(copy_byte_to_bool_disjunct_layout);
    }
}

