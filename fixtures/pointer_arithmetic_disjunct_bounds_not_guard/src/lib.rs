pub struct Control {
    ctrl: *const u8,
    bytes: usize,
}

impl Control {
    pub fn new(ctrl: *const u8, bytes: usize) -> Self {
        Self { ctrl, bytes }
    }

    fn num_ctrl_bytes(&self) -> usize {
        self.bytes
    }

    pub fn ctrl(&self, index: usize, allow_unchecked: bool) -> *const u8 {
        if index < self.num_ctrl_bytes() || allow_unchecked {
            // SAFETY: this fixture intentionally uses a disjunctive branch.
            unsafe { self.ctrl.add(index) }
        } else {
            core::ptr::null()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Control;

    #[test]
    fn ctrl_can_skip_index_guard() {
        let bytes = [0_u8; 4];
        let control = Control::new(bytes.as_ptr(), bytes.len());
        let _ = control.ctrl(8, true);
    }
}
