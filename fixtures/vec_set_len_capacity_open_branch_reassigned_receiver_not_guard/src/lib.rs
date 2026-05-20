pub fn maybe_extend_len(mut values: &mut Vec<u8>, fallback: &mut Vec<u8>, new_len: usize) {
    if new_len <= values.capacity() {
        values = fallback;
        // SAFETY: this comment is intentionally stale because values is reassigned.
        unsafe { values.set_len(new_len) }
    }
}

#[cfg(test)]
mod tests {
    use super::maybe_extend_len;

    #[test]
    fn branch_mentions_owner_without_executing_reassigned_receiver_path() {
        let mut values = Vec::with_capacity(4);
        let mut fallback = Vec::with_capacity(1);
        values.push(1);
        fallback.push(2);

        maybe_extend_len(&mut values, &mut fallback, 5);

        assert_eq!(values.len(), 1);
        assert_eq!(fallback.len(), 1);
    }
}
