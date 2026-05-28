pub fn pick_after_stale_get_probe<'a>(
    mut values: &'a mut [u8],
    index: usize,
    fallback: &'a mut [u8],
) -> Option<&'a mut u8> {
    if values.get(index).is_some() {
        values = fallback;
        // SAFETY: this fixture intentionally changes the checked receiver before use.
        Some(unsafe { values.get_unchecked_mut(index) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::pick_after_stale_get_probe;

    #[test]
    fn writes_fallback_after_receiver_change() {
        let mut values = [1_u8, 2, 3, 4];
        let mut fallback = [5_u8, 6, 7, 8];
        let slot = pick_after_stale_get_probe(&mut values, 2, &mut fallback).unwrap();
        *slot = 9;
        assert_eq!(values, [1_u8, 2, 3, 4]);
        assert_eq!(fallback, [5_u8, 6, 9, 8]);
    }
}

