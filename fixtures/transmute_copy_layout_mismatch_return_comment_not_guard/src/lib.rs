pub fn copy_byte_to_bool_layout_commented_return(value: u8) -> bool {
    if core::mem::size_of::<u8>() != core::mem::size_of::<bool>() {
        // return false before transmute_copy when source and destination layouts mismatch.
        observe_layout_mismatch();
    }
    // SAFETY: this fixture intentionally leaves the layout mismatch branch without an executable return guard.
    unsafe { core::mem::transmute_copy::<u8, bool>(&value) }
}

fn observe_layout_mismatch() {}

#[cfg(test)]
mod tests {
    use super::copy_byte_to_bool_layout_commented_return;

    #[test]
    fn copies_known_bool_byte() {
        let _value = copy_byte_to_bool_layout_commented_return(1);
    }
}
