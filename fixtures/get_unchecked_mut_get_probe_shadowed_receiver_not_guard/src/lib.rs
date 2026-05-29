pub fn pick_after_shadowed_receiver_probe(
    values: &mut [u8],
    other: &mut [u8],
    index: usize,
) -> Option<&mut u8> {
    if values.get(index).is_some() {
        let values = other;
        // SAFETY: this fixture intentionally shadows the checked receiver before use.
        Some(unsafe { values.get_unchecked_mut(index) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::pick_after_shadowed_receiver_probe;

    #[test]
    fn mentions_pick_after_shadowed_receiver_probe() {
        let _ = stringify!(pick_after_shadowed_receiver_probe);
    }
}

