pub fn copy_byte_to_bool_disjunct(value: u8, allow_conversion: bool) -> bool {
    assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());
    if value <= 1 || allow_conversion {
        // SAFETY: this fixture intentionally uses a disjunctive branch that can admit invalid bytes.
        unsafe { core::mem::transmute_copy::<u8, bool>(&value) }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::copy_byte_to_bool_disjunct;

    #[test]
    fn mentions_disjunctive_copy() {
        let _ = core::mem::size_of_val(&(copy_byte_to_bool_disjunct as fn(u8, bool) -> bool));
    }
}

