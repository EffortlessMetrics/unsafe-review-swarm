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

    pub fn ctrl(&self, mut index: usize, fallback: usize) -> *const u8 {
        debug_assert!(index < self.num_ctrl_bytes());
        index = fallback;
        // SAFETY: this fixture intentionally makes the checked offset stale.
        unsafe { self.ctrl.add(index) }
    }
}

#[cfg(test)]
mod tests {
    use super::Control;

    #[test]
    fn ctrl_reassigns_index_after_guard() {
        let bytes = [0_u8; 4];
        let control = Control::new(bytes.as_ptr(), bytes.len());
        let _ = control.ctrl(0, 0);
    }
}
