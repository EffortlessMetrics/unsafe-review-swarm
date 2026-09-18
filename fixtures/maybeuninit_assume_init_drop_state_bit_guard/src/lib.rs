use core::mem::MaybeUninit;

const WRITE: usize = 0x1;

struct Slot {
    state: usize,
    msg: MaybeUninit<String>,
}

pub fn drop_written(slot: &mut Slot) {
    // SAFETY: this fixture exposes a state-bit guard dominating assume_init_drop.
    if slot.state.get_mut() & WRITE != 0 {
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
    use super::{drop_written, Slot, WRITE};
    use core::mem::MaybeUninit;

    #[test]
    fn drops_written_slot() {
        let mut slot = Slot {
            state: WRITE,
            msg: MaybeUninit::new(String::from("initialized")),
        };
        drop_written(&mut slot);
    }
}
