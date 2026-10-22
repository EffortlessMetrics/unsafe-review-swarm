//! Shared/ambiguous source: included into the library via `#[path]`, so
//! its role cannot be decided from the directory name alone.

/// Shared seam: unchecked arithmetic helper compiled into production.
pub fn advance(ptr: *mut u8, n: usize) -> *mut u8 {
    unsafe { ptr.add(n) }
}
