use core::mem::MaybeUninit;

pub(crate) fn initialize_first_element(slot: &mut MaybeUninit<[u8; 4]>, value: u8) {
    unsafe {
        (*slot.as_mut_ptr())[0] = value;
    }
}

