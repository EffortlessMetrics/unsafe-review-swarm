pub fn try_reserve_with_stale_additional(
    values: &mut Vec<u8>,
    mut additional: usize,
) -> Result<(), std::collections::TryReserveError> {
    let new_len = values.len() + additional;
    additional = 0;
    values.try_reserve(additional)?;
    // SAFETY: this fixture try_reserves with a stale additional amount and omits initialization.
    unsafe { values.set_len(new_len) }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::try_reserve_with_stale_additional;

    #[test]
    fn mentions_try_reserve_with_stale_additional() {
        let mut values = vec![1_u8];
        try_reserve_with_stale_additional(&mut values, 0).unwrap();
    }
}
