use core::mem::MaybeUninit;

pub fn set_from_unrelated_cap<const CAP: usize>(values: &mut Vec<u8>, requested: usize) {
    let _scratch = [MaybeUninit::<u8>::uninit(); CAP];
    let cap = requested;
    // SAFETY: this fixture mentions an unrelated const-CAP buffer but does not bound values capacity.
    unsafe { values.set_len(cap) }
}

#[cfg(test)]
mod tests {
    use super::set_from_unrelated_cap;

    #[test]
    fn mentions_set_from_unrelated_cap() {
        let mut values = Vec::with_capacity(1);
        set_from_unrelated_cap::<1>(&mut values, 0);
    }
}
