use core::mem::MaybeUninit;

use super::Pair;

pub(crate) fn initialize_first_field(slot: &mut MaybeUninit<Pair>, value: u32) {
    unsafe {
        (*slot.as_mut_ptr()).first = value;
    }
}

