pub fn byte_to_bool_closed_layout(value: u8) -> bool {
    if value == 0 {
        assert_eq!(core::mem::size_of::<u8>(), core::mem::size_of::<bool>());
    }
    // SAFETY: this fixture checks layout in a closed branch and lacks bool-domain evidence.
    unsafe { core::mem::transmute::<u8, bool>(value) }
}

#[cfg(test)]
mod tests {
    use super::byte_to_bool_closed_layout;

    #[test]
    fn mentions_byte_to_bool_closed_layout() {
        let _ = stringify!(byte_to_bool_closed_layout);
    }
}
