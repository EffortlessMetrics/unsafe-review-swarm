pub fn pick_after_shadowed_get_probe(values: &mut [u8], index: usize) -> Option<&mut u8> {
    if values.get(index).is_some() {
        let index = values.len();
        // SAFETY: this fixture intentionally shadows the checked index before use.
        Some(unsafe { values.get_unchecked_mut(index) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::pick_after_shadowed_get_probe;

    #[test]
    fn mentions_pick_after_shadowed_get_probe() {
        let _ = stringify!(pick_after_shadowed_get_probe);
    }
}

