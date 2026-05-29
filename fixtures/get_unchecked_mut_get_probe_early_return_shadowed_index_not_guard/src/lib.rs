pub fn pick_after_shadowed_get_probe_return_index(
    values: &mut [u8],
    index: usize,
) -> Option<&mut u8> {
    if values.get(index).is_none() {
        return None;
    }
    let index = values.len();
    // SAFETY: this fixture intentionally shadows the checked index before use.
    Some(unsafe { values.get_unchecked_mut(index) })
}

#[cfg(test)]
mod tests {
    use super::pick_after_shadowed_get_probe_return_index;

    #[test]
    fn mentions_pick_after_shadowed_get_probe_return_index() {
        let _ = stringify!(pick_after_shadowed_get_probe_return_index);
    }
}
