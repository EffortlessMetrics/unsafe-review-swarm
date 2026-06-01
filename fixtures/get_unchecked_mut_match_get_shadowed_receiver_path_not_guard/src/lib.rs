pub struct Bag<'a> {
    pub values: &'a mut [u8],
}

pub fn pick_after_shadowed_match_get_path<'a>(
    bag: Bag<'a>,
    index: usize,
    fallback: &'a mut [u8],
) -> Option<&'a mut u8> {
    match bag.values.get(index) {
        Some(_) => {
            let bag = Bag { values: fallback };
            // SAFETY: this fixture intentionally shadows the checked receiver path before use.
            Some(unsafe { bag.values.get_unchecked_mut(index) })
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{pick_after_shadowed_match_get_path, Bag};

    #[test]
    fn writes_fallback_after_receiver_path_shadowing() {
        let mut values = [1_u8, 2, 3, 4];
        let mut fallback = [5_u8, 6, 7, 8];
        {
            let bag = Bag {
                values: &mut values,
            };
            let slot = pick_after_shadowed_match_get_path(bag, 2, &mut fallback).unwrap();
            *slot = 9;
        }
        assert_eq!(values, [1_u8, 2, 3, 4]);
        assert_eq!(fallback, [5_u8, 6, 9, 8]);
    }
}
