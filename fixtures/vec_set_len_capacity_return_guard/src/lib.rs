pub fn extend_len(values: &mut Vec<u8>, new_len: usize) {
    if new_len > values.capacity() {
        return;
    }
    // SAFETY: capacity is checked above, but this fixture does not initialize new elements.
    unsafe { values.set_len(new_len) }
}

#[cfg(test)]
mod tests {
    use super::extend_len;

    #[test]
    fn extends_len_after_capacity_return_guard() {
        let mut values = Vec::with_capacity(4);
        values.push(1);
        extend_len(&mut values, 1);
    }
}
