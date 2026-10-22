use std::ffi::c_char;

unsafe extern "C" {
    fn ffi_lookup(key: *const c_char) -> i32;
}

/// Null is a valid not-found sentinel; no check is required or wanted.
pub fn sentinel(key: *const c_char) -> i32 {
    unsafe { ffi_lookup(key) }
}

unsafe extern "C" {
    fn ffi_copy(dst: *mut u8, src: *const u8, n: usize);
}

/// Null is valid exactly when `n == 0`.
pub fn len_coupled(dst: *mut u8, src: *const u8) {
    unsafe { ffi_copy(dst, src, 0) }
}

unsafe extern "C" {
    fn ffi_maybe_out(x: i32, out: *mut i32) -> i32;
}

/// A null out-pointer means "no output wanted".
pub fn optional_out(x: i32) -> i32 {
    let out: *mut i32 = std::ptr::null_mut();
    unsafe { ffi_maybe_out(x, out) }
}

pub struct Opaque {
    _private: u32,
}

unsafe extern "C" {
    fn ffi_handle_use(h: *const Opaque) -> i32;
}

/// Opaque handles carry API-specific validity, not nullability.
pub fn opaque(h: *const Opaque) -> i32 {
    unsafe { ffi_handle_use(h) }
}

unsafe extern "C" {
    fn ffi_two(a: *const u8, b: *const u8) -> u8;
}

/// Only `a` is checked; a check on one argument must not invent a
/// requirement for the other (or for itself).
pub fn different_arg(a: *const u8, b: *const u8) -> u8 {
    if a.is_null() {
        return 0;
    }
    unsafe { ffi_two(a, b) }
}

/// A dominating check with no declaration in context is an observed fact,
/// not a requirement source.
pub fn guarded_libc(ptr: *const i8) -> usize {
    if ptr.is_null() {
        return 0;
    }
    unsafe { libc::strlen(ptr) }
}

#[cfg(test)]
mod tests {
    use super::{different_arg, guarded_libc, len_coupled, opaque, optional_out, sentinel};

    #[test]
    fn mentions_wrappers() {
        let _sentinel = sentinel as fn(*const std::ffi::c_char) -> i32;
        let _len_coupled = len_coupled as fn(*mut u8, *const u8) -> ();
        let _optional_out = optional_out as fn(i32) -> i32;
        let _opaque = opaque as fn(*const super::Opaque) -> i32;
        let _different_arg = different_arg as fn(*const u8, *const u8) -> u8;
        let _guarded_libc = guarded_libc as fn(*const i8) -> usize;
    }
}
