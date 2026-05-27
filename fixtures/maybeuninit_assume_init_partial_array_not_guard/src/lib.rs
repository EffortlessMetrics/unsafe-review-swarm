use core::mem::MaybeUninit;

mod partial;

pub fn assume_partially_initialized_array(value: u8) -> [u8; 4] {
    let mut slot = MaybeUninit::<[u8; 4]>::uninit();
    partial::initialize_first_element(&mut slot, value);
    // SAFETY: first element was initialized above.
    unsafe { slot.assume_init() }
}

#[cfg(test)]
mod tests {
    use super::assume_partially_initialized_array;

    #[test]
    fn mentions_assume_partially_initialized_array() {
        let _ = core::mem::size_of_val(
            &(assume_partially_initialized_array as fn(u8) -> [u8; 4]),
        );
    }
}
