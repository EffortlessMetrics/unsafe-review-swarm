/// A local module whose inner path `something::libc::foo()` contains `libc`
/// as an inner segment, not as the root of a foreign-libc path.
/// The FFI boundary heuristic must not route this as a foreign-libc call.
mod something {
    pub mod libc {
        /// # Safety
        ///
        /// Caller must pass a valid pointer.
        pub unsafe fn strlen(_ptr: *const i8) -> usize {
            0
        }
    }
}

pub fn len(ptr: *const i8) -> usize {
    // SAFETY: delegates to a local Rust module, not a foreign boundary.
    unsafe { something::libc::strlen(ptr) }
}

#[cfg(test)]
mod tests {
    use super::len;

    #[test]
    fn mentions_len() {
        let _f = len as fn(*const i8) -> usize;
    }
}
