pub fn reserve_then_extend(values: &mut Vec<u8>, additional: usize) {
    let new_len = values.len() + additional;
    values.reserve(additional);
    // SAFETY: reserve proves capacity for the new length, but this fixture intentionally omits initialization.
    unsafe { values.set_len(new_len) }
}

#[cfg(test)]
mod tests {
    use super::reserve_then_extend;

    #[test]
    fn mentions_reserve_then_extend() {
        let mut values = vec![1_u8];
        reserve_then_extend(&mut values, 0);
    }
}
