pub fn pick_after_let_else_get(values: &mut [u8], index: usize) -> Option<&mut u8> {
    let Some(_) = values.get(index) else {
        return None;
    };
    // SAFETY: the get probe above returned early when `index` was out of bounds.
    Some(unsafe { values.get_unchecked_mut(index) })
}

#[cfg(test)]
mod tests {
    use super::pick_after_let_else_get;

    #[test]
    fn picks_after_let_else_get() {
        let mut values = [1_u8, 2, 3];
        let picked = pick_after_let_else_get(&mut values, 1).expect("in bounds");
        *picked = 9;
    }
}
