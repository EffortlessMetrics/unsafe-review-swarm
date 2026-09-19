use std::ffi::c_char;

unsafe extern "C" {
    fn ffi_strlen(s: *const c_char) -> usize;
    fn ffi_add(a: i32, b: i32) -> i32;
}

pub fn unguarded(s: *const c_char) -> usize {
    unsafe { ffi_strlen(s) }
}

pub fn added(a: i32, b: i32) -> i32 {
    unsafe { ffi_add(a, b) }
}

#[cfg(test)]
mod tests {
    use super::{added, unguarded};

    #[test]
    fn mentions_wrappers() {
        let _unguarded = unguarded as fn(*const std::ffi::c_char) -> usize;
        let _added = added as fn(i32, i32) -> i32;
    }
}
