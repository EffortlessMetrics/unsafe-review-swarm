use core::mem::MaybeUninit;

const WRITE: usize = 0x1;

struct Slot {
    state: usize,
    msg: MaybeUninit<String>,
}

pub fn drop_when_unset(slot: &mut Slot) {
    // SAFETY: fixture checks that the bit-unset arm is not evidence.
    if slot.state.get_mut() & WRITE == 0 {
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
    use super::drop_when_unset;

    #[test]
    fn mentions_drop_when_unset() {
        let _ = core::mem::size_of_val(&(drop_when_unset as fn(&mut super::Slot)));
    }
}
