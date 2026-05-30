pub type StaticMutBytes = &'static mut [u8];

pub fn take_static_mut(bytes: StaticMutBytes) -> usize {
    bytes.len()
}
