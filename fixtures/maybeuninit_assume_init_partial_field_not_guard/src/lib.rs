use core::mem::MaybeUninit;

mod partial;

#[repr(C)]
pub struct Pair {
    first: u32,
    second: u32,
}

pub fn assume_partially_initialized(value: u32) -> Pair {
    let mut slot = MaybeUninit::<Pair>::uninit();
    partial::initialize_first_field(&mut slot, value);
    // SAFETY: first field was initialized above.
    unsafe { slot.assume_init() }
}

#[cfg(test)]
mod tests {
    use super::{assume_partially_initialized, Pair};

    #[test]
    fn mentions_assume_partially_initialized() {
        let _ = core::mem::size_of_val(&(assume_partially_initialized as fn(u32) -> Pair));
    }
}
