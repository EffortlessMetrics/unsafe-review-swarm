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

    pub fn ctrl(&self, index: usize) -> Option<*const u8> {
        // SAFETY: this fixture intentionally checks bounds after pointer arithmetic.
        let ptr = unsafe { self.ctrl.add(index) };
        if index >= self.num_ctrl_bytes() {
            return None;
        }
        Some(ptr)
    }
}

#[cfg(test)]
mod tests {
    use super::Control;

    #[test]
    fn ctrl_checks_after_add() {
        let bytes = [0_u8; 4];
        let control = Control::new(bytes.as_ptr(), bytes.len());
        let _ = control.ctrl(0);
    }
}

