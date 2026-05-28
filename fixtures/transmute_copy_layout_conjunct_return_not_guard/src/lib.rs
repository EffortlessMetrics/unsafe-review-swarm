pub fn copy_byte_to_bool_conjunct_layout_return(value: u8, allow_conversion: bool) -> bool {
    if core::mem::size_of::<u8>() != core::mem::size_of::<bool>() && allow_conversion {
        return false;
    }
    // SAFETY: a conjunctive mismatch branch can fall through when layout differs and allow_conversion is false.
    unsafe { core::mem::transmute_copy::<u8, bool>(&value) }
}

#[cfg(test)]
mod tests {
    use super::copy_byte_to_bool_conjunct_layout_return;

    #[test]
    fn mentions_copy_byte_to_bool_conjunct_layout_return() {
        let _ = stringify!(copy_byte_to_bool_conjunct_layout_return);
    }
}

