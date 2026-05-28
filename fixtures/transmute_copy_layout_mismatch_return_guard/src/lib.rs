pub fn copy_byte_to_bool_layout_mismatch_return(value: u8) -> bool {
    if core::mem::size_of::<u8>() != core::mem::size_of::<bool>() {
        return false;
    }
    // SAFETY: the mismatch branch exits before this call, but this fixture intentionally lacks a valid-value proof that value is 0 or 1.
    unsafe { core::mem::transmute_copy::<u8, bool>(&value) }
}

#[cfg(test)]
mod tests {
    use super::copy_byte_to_bool_layout_mismatch_return;

    #[test]
    fn copies_known_bool_byte() {
        let _value = copy_byte_to_bool_layout_mismatch_return(1);
    }
}

