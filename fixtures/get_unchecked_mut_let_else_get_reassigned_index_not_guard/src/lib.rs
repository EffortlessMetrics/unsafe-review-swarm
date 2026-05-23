pub fn pick_after_stale_let_else_get(values: &mut [u8], mut index: usize) -> Option<&mut u8> {
    let Some(_) = values.get(index) else {
        return None;
    };
    index = values.len();
    // SAFETY: this fixture intentionally changes the checked index before use.
    Some(unsafe { values.get_unchecked_mut(index) })
}

#[cfg(test)]
mod tests {
    use super::pick_after_stale_let_else_get;

    #[test]
    fn mentions_pick_after_stale_let_else_get() {
        let _ = stringify!(pick_after_stale_let_else_get);
    }
}
