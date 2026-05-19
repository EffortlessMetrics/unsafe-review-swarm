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

    pub fn ctrl(&self, index: usize, other: usize) -> *const u8 {
        debug_assert!(other < self.num_ctrl_bytes());
        // SAFETY: this fixture intentionally checks a different index.
        unsafe { self.ctrl.add(index) }
    }
}

#[cfg(test)]
mod tests {
    use super::Control;

    #[test]
    fn ctrl_uses_index_guard() {
        let bytes = [0_u8; 4];
        let control = Control::new(bytes.as_ptr(), bytes.len());
        let _ = control.ctrl(0, 0);
    }
}

