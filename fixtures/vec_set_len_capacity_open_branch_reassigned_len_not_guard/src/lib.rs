pub fn maybe_extend_len(values: &mut Vec<u8>, mut new_len: usize) {
    if new_len <= values.capacity() {
        new_len = values.capacity() + 1;
        // SAFETY: this comment is intentionally stale because new_len is reassigned.
        unsafe { values.set_len(new_len) }
    }
}

#[cfg(test)]
mod tests {
    use super::maybe_extend_len;

    #[test]
    fn branch_mentions_owner_without_executing_reassigned_len_path() {
        let mut values = Vec::with_capacity(4);
        values.push(1);

        maybe_extend_len(&mut values, 5);

        assert_eq!(values.len(), 1);
    }
}
