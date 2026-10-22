//! Example seam: benchmark/demo helper, not shipped library code.

/// Example seam: raw write in a demo harness.
pub fn demo_write(ptr: *mut u8, value: u8) {
    unsafe { *ptr = value }
}

fn main() {}
