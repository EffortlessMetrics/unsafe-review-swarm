pub fn pick_after_shadowed_if_let_get_index(
    values: &mut [u8],
    index: usize,
) -> Option<&mut u8> {
    if let Some(_) = values.get(index) {
        let index = values.len();
        // SAFETY: this fixture intentionally shadows the checked index before use.
        Some(unsafe { values.get_unchecked_mut(index) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::pick_after_shadowed_if_let_get_index;

    #[test]
    fn mentions_pick_after_shadowed_if_let_get_index() {
        let _ = stringify!(pick_after_shadowed_if_let_get_index);
    }
}
