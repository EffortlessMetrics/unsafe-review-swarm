unsafe extern "C" {
    fn zstd_compress_into(
        src: *const u8,
        src_len: usize,
        dst: *mut u8,
        dst_len: usize,
    ) -> usize;
}

pub fn zstd_overlap_handoff(input: &[u8], output: &mut [u8]) -> usize {
    // SAFETY: this fixture intentionally documents the native Zstd-style FFI seam;
    // it does not prove caller-provided JS-backed input/output spans are disjoint.
    unsafe { zstd_compress_into(input.as_ptr(), input.len(), output.as_mut_ptr(), output.len()) }
}

#[cfg(test)]
mod tests {
    use super::zstd_overlap_handoff;

    #[test]
    fn mentions_zstd_overlap_handoff() {
        let _handoff = zstd_overlap_handoff as fn(&[u8], &mut [u8]) -> usize;
    }
}
