pub fn pick_after_other_get_probe<'a>(
    values: &'a mut [u8],
    index: usize,
    other: &'a mut [u8],
) -> Option<&'a mut u8> {
    if values.get(index).is_some() {
        // SAFETY: this fixture intentionally checks `values` but indexes `other`.
        Some(unsafe { other.get_unchecked_mut(index) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::pick_after_other_get_probe;

    #[test]
    fn writes_other_after_values_probe() {
        let mut values = [1_u8, 2, 3, 4];
        let mut other = [5_u8, 6, 7, 8];
        let slot = pick_after_other_get_probe(&mut values, 2, &mut other).unwrap();
        *slot = 9;
        assert_eq!(values, [1_u8, 2, 3, 4]);
        assert_eq!(other, [5_u8, 6, 9, 8]);
    }
}
