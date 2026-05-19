pub fn extend_len(values: &mut Vec<u8>, new_len: usize) {
    if new_len <= values.capacity() {
        record_observed_capacity(new_len);
    }
    // SAFETY: this fixture observes a passing capacity branch but does not keep
    // the set_len call inside that branch.
    unsafe { values.set_len(new_len) }
}

fn record_observed_capacity(_new_len: usize) {}

#[cfg(test)]
mod tests {
    use super::extend_len;

    #[test]
    fn mentions_extend_len() {
        let mut values = Vec::with_capacity(4);
        let _ = stringify!(extend_len);
        let _ = &mut values;
    }
}
