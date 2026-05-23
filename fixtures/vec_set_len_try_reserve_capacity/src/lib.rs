pub fn try_reserve_then_extend(
    values: &mut Vec<u8>,
    additional: usize,
) -> Result<(), std::collections::TryReserveError> {
    let new_len = values.len() + additional;
    values.try_reserve(additional)?;
    // SAFETY: try_reserve proves capacity on the continuing path, but this fixture intentionally omits initialization.
    unsafe { values.set_len(new_len) }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::try_reserve_then_extend;

    #[test]
    fn mentions_try_reserve_then_extend() {
        let mut values = vec![1_u8];
        try_reserve_then_extend(&mut values, 0).unwrap();
    }
}
