pub struct Bag<'a> {
    pub values: &'a mut [u8],
}

pub fn pick_after_stale_get_probe_path<'a>(
    mut bag: Bag<'a>,
    index: usize,
    fallback: &'a mut [u8],
) -> Option<&'a mut u8> {
    if bag.values.get(index).is_some() {
        bag.values = fallback;
        // SAFETY: this fixture intentionally changes the checked receiver path before use.
        Some(unsafe { bag.values.get_unchecked_mut(index) })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{pick_after_stale_get_probe_path, Bag};

    #[test]
    fn writes_fallback_after_receiver_path_change() {
        let mut values = [1_u8, 2, 3, 4];
        let mut fallback = [5_u8, 6, 7, 8];
        {
            let bag = Bag {
                values: &mut values,
            };
            let slot = pick_after_stale_get_probe_path(bag, 2, &mut fallback).unwrap();
            *slot = 9;
        }
        assert_eq!(values, [1_u8, 2, 3, 4]);
        assert_eq!(fallback, [5_u8, 6, 9, 8]);
    }
}
