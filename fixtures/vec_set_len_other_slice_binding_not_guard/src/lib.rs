use core::mem::MaybeUninit;

pub fn extend_from_other_slice(
    values: &mut Vec<MaybeUninit<u8>>,
    other: &mut [MaybeUninit<u8>],
    bytes: &[u8],
) {
    let old_len = values.len();
    let new_len = old_len + bytes.len();
    if new_len > values.capacity() {
        return;
    }
    let dst = &mut other[old_len..new_len];
    for (dst, src) in dst.iter_mut().zip(bytes.iter()) {
        *dst = MaybeUninit::new(*src);
    }
    // SAFETY: capacity is checked above, but the initialized slice is not `values`.
    unsafe {
        values.set_len(new_len);
    }
}

#[cfg(test)]
mod tests {
    use super::extend_from_other_slice;

    #[test]
    fn mentions_extend_from_other_slice() {
        let mut values = Vec::with_capacity(0);
        let mut other = [];
        extend_from_other_slice(&mut values, &mut other, b"");
    }
}
