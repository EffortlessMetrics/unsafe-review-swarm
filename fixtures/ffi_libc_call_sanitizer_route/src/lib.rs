pub fn len(ptr: *const i8) -> usize {
    // SAFETY: caller provides a C string pointer for the libc strlen contract.
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

