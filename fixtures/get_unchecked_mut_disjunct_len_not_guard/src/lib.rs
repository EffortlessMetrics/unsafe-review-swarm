pub fn pick_when_allowed_or_checked(values: &mut [u8], index: usize, allow: bool) -> Option<&mut u8> {
    if index < values.len() || allow {
        // SAFETY: `allow` is not bounds evidence for `values[index]`.
        Some(unsafe { values.get_unchecked_mut(index) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::pick_when_allowed_or_checked;

    #[test]
    fn picks_checked_value() {
        let mut values = [1_u8, 2, 3];
        let picked = pick_when_allowed_or_checked(&mut values, 1, false).expect("in bounds");
        *picked = 9;
    }
}
