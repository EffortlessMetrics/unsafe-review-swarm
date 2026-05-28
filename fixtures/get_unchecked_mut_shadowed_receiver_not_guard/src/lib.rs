pub fn pick_after_receiver_shadow<'a>(
    values: &'a mut [u8],
    index: usize,
    fallback: &'a mut [u8],
) -> Option<&'a mut u8> {
    if index >= values.len() {
        return None;
    }
    let values = fallback;
    // SAFETY: this fixture intentionally shadows the checked receiver before use.
    Some(unsafe { values.get_unchecked_mut(index) })
}

#[cfg(test)]
mod tests {
    use super::pick_after_receiver_shadow;

    #[test]
    fn mentions_pick_after_receiver_shadow() {
        let _ = stringify!(pick_after_receiver_shadow);
    }
}

