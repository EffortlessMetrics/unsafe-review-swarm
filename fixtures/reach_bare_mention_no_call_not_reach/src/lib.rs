// Negative-control (call-shape): `Collector` appears in a comment inside
// the inline test module but is never called or constructed there.
// A bare identifier mention must NOT credit test reach.

pub struct Collector {
    ptr: *mut u8,
}

// SAFETY: fixture states the Send promise but provides no Loom witness.
unsafe impl Send for Collector {}

#[cfg(test)]
mod tests {
    #[test]
    fn check_zero() {
        // Collector is mentioned but not called here.
        assert_eq!(0, 0);
    }
}
