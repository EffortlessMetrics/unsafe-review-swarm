pub fn reserve_with_stale_additional(values: &mut Vec<u8>, mut additional: usize) {
    let new_len = values.len() + additional;
    additional = 0;
    values.reserve(additional);
    // SAFETY: this fixture reserves with a stale additional amount and omits initialization.
    unsafe { values.set_len(new_len) }
}

#[cfg(test)]
mod tests {
    use super::reserve_with_stale_additional;

    #[test]
    fn mentions_reserve_with_stale_additional() {
        let mut values = vec![1_u8];
        reserve_with_stale_additional(&mut values, 0);
    }
}
