/// This fixture is an adversarial probe for the shrink-evidence heuristic.
/// A benign read of the current length (let _old = values.len()) appears in
/// the same context as a capacity-only guard (new_len <= values.capacity()).
/// Together they contain both the substring "new_len<=" and "{receiver}.len()"
/// but NOT the joined predicate "new_len<=values.len()".  The tool must NOT
/// treat this as shrink evidence and must still emit the initialized_memory card.
pub fn unsafe_shrink_cap_guard_with_len_read(values: &mut Vec<u8>, new_len: usize) {
    // Benign read of current length — not a guard on new_len.
    let _old = values.len();
    // SAFETY: new_len is at most capacity — but capacity can exceed the
    // initialized range, so this is insufficient initialization evidence.
    if new_len <= values.capacity() {
        unsafe {
            values.set_len(new_len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::unsafe_shrink_cap_guard_with_len_read;

    #[test]
    fn probe_cap_guard_with_len_read() {
        let mut values = vec![1_u8, 2, 3];
        unsafe_shrink_cap_guard_with_len_read(&mut values, 2);
    }
}
