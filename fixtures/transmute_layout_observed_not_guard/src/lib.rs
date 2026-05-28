pub fn byte_to_bool_observed_layout(value: u8) -> bool {
    let _same_layout = core::mem::size_of::<u8>() == core::mem::size_of::<bool>();
    // SAFETY: this fixture observes layout equality but does not assert or branch on it.
    unsafe { core::mem::transmute::<u8, bool>(value) }
}

#[cfg(test)]
mod tests {
    use super::byte_to_bool_observed_layout;

    #[test]
    fn mentions_byte_to_bool_observed_layout() {
        let _ = stringify!(byte_to_bool_observed_layout);
    }
}
