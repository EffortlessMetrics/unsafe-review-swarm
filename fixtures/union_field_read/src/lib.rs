pub union Word {
    bits: u64,
    bytes: [u8; 8],
}

pub fn read_bits(w: Word) -> u64 {
    // SAFETY: fixture keeps the active-variant question visible but omits validity proof.
    unsafe { w.bits }
}

#[cfg(test)]
mod tests {
    use super::read_bits;

    #[test]
    fn mentions_read_bits() {
        let _ = read_bits as fn(crate::Word) -> u64;
    }
}
