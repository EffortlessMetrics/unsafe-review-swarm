pub fn pick_after_index_shadow(values: &mut [u8], index: usize) -> Option<&mut u8> {
    if index >= values.len() {
        return None;
    }
    let index = values.len();
    // SAFETY: this fixture intentionally shadows the checked index before use.
    Some(unsafe { values.get_unchecked_mut(index) })
}

#[cfg(test)]
mod tests {
    use super::pick_after_index_shadow;

    #[test]
    fn mentions_pick_after_index_shadow() {
        let _ = stringify!(pick_after_index_shadow);
    }
}

