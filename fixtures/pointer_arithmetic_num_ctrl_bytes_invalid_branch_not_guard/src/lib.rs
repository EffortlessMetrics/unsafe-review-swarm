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

    pub fn ctrl(&self, index: usize) -> *const u8 {
        if index >= self.num_ctrl_bytes() {
            // SAFETY: this fixture intentionally places the operation in the invalid branch.
            unsafe { self.ctrl.add(index) }
        } else {
            self.ctrl
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Control;

    #[test]
    fn ctrl_uses_invalid_branch() {
        let bytes = [0_u8; 4];
        let control = Control::new(bytes.as_ptr(), bytes.len());
        let _ = control.ctrl(0);
    }
}
