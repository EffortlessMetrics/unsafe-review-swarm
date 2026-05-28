pub fn pick_after_index_increment(values: &mut [u8], mut index: usize) -> Option<&mut u8> {
    if index >= values.len() {
        return None;
    }
    index += 1;
    // SAFETY: this fixture intentionally mutates the checked index before use.
    Some(unsafe { values.get_unchecked_mut(index) })
}

#[cfg(test)]
mod tests {
    use super::pick_after_index_increment;

    #[test]
    fn mentions_pick_after_index_increment() {
        let _ = stringify!(pick_after_index_increment);
    }
}
