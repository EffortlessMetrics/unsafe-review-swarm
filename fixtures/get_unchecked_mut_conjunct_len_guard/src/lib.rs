pub fn pick_checked_when_allowed(values: &mut [u8], index: usize, allow: bool) -> Option<&mut u8> {
    if index < values.len() && allow {
        // SAFETY: `index` was checked against `values.len()` in this branch.
        Some(unsafe { values.get_unchecked_mut(index) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::pick_checked_when_allowed;

    #[test]
    fn picks_checked_value_when_allowed() {
        let mut values = [1_u8, 2, 3];
        let picked = pick_checked_when_allowed(&mut values, 1, true).expect("in bounds");
        *picked = 9;
    }
}
