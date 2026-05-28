mod ffi {
    unsafe extern "C" {
        pub(super) fn strlen(ptr: *const u8) -> usize;
    }
}

pub fn len(ptr: *const u8) -> usize {
    // SAFETY: caller provides a C string pointer for the external strlen contract.
    unsafe { ffi::strlen(ptr) }
}

#[cfg(test)]
mod tests {
    use super::len;

    #[test]
    fn mentions_len_wrapper() {
        let _wrapper = len as fn(*const u8) -> usize;
    }
}

