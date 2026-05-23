pub fn pick_after_stale_get_probe(values: &mut [u8], mut index: usize) -> Option<&mut u8> {
    if values.get(index).is_some() {
        index = values.len();
        // SAFETY: this fixture intentionally changes the checked index before use.
        Some(unsafe { values.get_unchecked_mut(index) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::pick_after_stale_get_probe;

    #[test]
    fn mentions_pick_after_stale_get_probe() {
        let _ = stringify!(pick_after_stale_get_probe);
    }
}

