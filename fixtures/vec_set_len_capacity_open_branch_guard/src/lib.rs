pub fn maybe_extend_len(values: &mut Vec<u8>, new_len: usize) {
    if new_len <= values.capacity() {
        // SAFETY: capacity is checked by the branch, but new elements are not initialized.
        unsafe { values.set_len(new_len) }
    }
}

#[cfg(test)]
mod tests {
    use super::maybe_extend_len;

    #[test]
    fn extends_len_inside_capacity_branch() {
        let mut values = Vec::with_capacity(4);
        values.push(1);

        maybe_extend_len(&mut values, 1);

        assert_eq!(values.len(), 1);
    }
}
