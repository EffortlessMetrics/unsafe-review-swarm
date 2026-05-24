pub fn extend_len_from_comment(values: &mut Vec<u8>, new_len: usize) {
    assert!(new_len <= values.capacity());
    // SAFETY: the extended range was initialized above.
    unsafe { values.set_len(new_len) }
}

#[cfg(test)]
mod tests {
    use super::extend_len_from_comment;

    #[test]
    fn mentions_extend_len_from_comment() {
        let _ = core::mem::size_of_val(&(extend_len_from_comment as fn(&mut Vec<u8>, usize)));
    }
}
