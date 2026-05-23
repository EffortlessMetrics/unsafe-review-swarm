pub fn pick_after_stale_match_get(values: &mut [u8], mut index: usize) -> Option<&mut u8> {
    match values.get(index) {
        Some(_) => {
            index = values.len();
            // SAFETY: this fixture intentionally changes the checked index before use.
            Some(unsafe { values.get_unchecked_mut(index) })
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::pick_after_stale_match_get;

    #[test]
    fn mentions_pick_after_stale_match_get() {
        let _ = stringify!(pick_after_stale_match_get);
    }
}
