/// Probe: transmute from u8 to bool with a layout (size_of) assertion but no
/// valid-value assertion. The size assertion discharges the layout obligation
/// but the valid-value obligation remains missing. The tool must report
/// guard_missing even though one obligation is met.
pub fn byte_to_bool_layout_only(value: u8) -> bool {
    assert_eq!(
        core::mem::size_of::<u8>(),
        core::mem::size_of::<bool>(),
        "layout sizes must match"
    );
    // SAFETY: sizes match — but this fixture deliberately omits the value
    // range check (value in {0, 1}) needed to establish bool validity.
    unsafe { core::mem::transmute::<u8, bool>(value) }
}

#[cfg(test)]
mod tests {
    use super::byte_to_bool_layout_only;

    #[test]
    fn probe_layout_only() {
        let _v = byte_to_bool_layout_only(1);
    }
}
