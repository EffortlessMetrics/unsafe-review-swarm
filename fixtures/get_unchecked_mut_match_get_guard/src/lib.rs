pub fn pick_after_match_get(values: &mut [u8], index: usize) -> Option<&mut u8> {
    match values.get(index) {
        Some(_) => {
            // SAFETY: the get probe matched Some, so `index` is in bounds for `values`.
            Some(unsafe { values.get_unchecked_mut(index) })
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::pick_after_match_get;

    #[test]
    fn picks_after_match_get() {
        let mut values = [1_u8, 2, 3];
        let picked = pick_after_match_get(&mut values, 1).expect("in bounds");
        *picked = 9;
    }
}
