pub fn pick_after_get_probe(values: &mut [u8], index: usize) -> Option<&mut u8> {
    if values.get(index).is_some() {
        // SAFETY: the get probe above proves `index` is in bounds for `values`.
        Some(unsafe { values.get_unchecked_mut(index) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::pick_after_get_probe;

    #[test]
    fn picks_after_get_probe() {
        let mut values = [1_u8, 2, 3];
        let picked = pick_after_get_probe(&mut values, 1).expect("in bounds");
        *picked = 9;
    }
}

