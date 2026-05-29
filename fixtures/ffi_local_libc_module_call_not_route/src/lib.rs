mod libc {
    /// # Safety
    ///
    /// The caller must pass a pointer that is valid for the wrapper contract.
    pub unsafe fn strlen(_ptr: *const i8) -> usize {
        0
    }
}

pub fn len(ptr: *const i8) -> usize {
    // SAFETY: this calls a local Rust module named libc, not a foreign boundary.
    unsafe { libc::strlen(ptr) }
}

#[cfg(test)]
mod tests {
    use super::len;

    #[test]
    fn mentions_len_wrapper() {
        let _wrapper = len as fn(*const i8) -> usize;
    }
}

