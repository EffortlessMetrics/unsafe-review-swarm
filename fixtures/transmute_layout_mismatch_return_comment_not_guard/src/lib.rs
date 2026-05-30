pub fn byte_to_bool_layout_commented_return(value: u8) -> bool {
    if core::mem::size_of::<u8>() != core::mem::size_of::<bool>() {
        // return false before transmute when source and destination layouts mismatch.
        observe_layout_mismatch();
    }
    // SAFETY: this fixture intentionally leaves the layout mismatch branch without an executable return guard.
    unsafe { core::mem::transmute::<u8, bool>(value) }
}

fn observe_layout_mismatch() {}

#[cfg(test)]
mod tests {
    use super::byte_to_bool_layout_commented_return;

    #[test]
    fn converts_known_bool_byte() {
        let _value = byte_to_bool_layout_commented_return(1);
    }
}
