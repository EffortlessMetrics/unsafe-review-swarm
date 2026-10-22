use core::mem::MaybeUninit;

const WRITE: usize = 0x1;

struct Slot {
    state: usize,
    msg: MaybeUninit<String>,
}

pub fn drop_unrelated_state(slot: &mut Slot, other: &mut Slot) {
    // SAFETY: fixture checks that a bit tested on another slot is not evidence.
    if other.state.get_mut() & WRITE != 0 {
        unsafe { (*slot.msg.get()).assume_init_drop() }
    }
}

trait StateAccess {
    fn get_mut(&mut self) -> &mut usize;
}

impl StateAccess for usize {
    fn get_mut(&mut self) -> &mut usize {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::drop_unrelated_state;

    #[test]
    fn mentions_drop_unrelated_state() {
        let _ = core::mem::size_of_val(
            &(drop_unrelated_state as fn(&mut super::Slot, &mut super::Slot)),
        );
    }
}
