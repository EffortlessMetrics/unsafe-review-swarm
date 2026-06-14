/// A function with an unsafe block that contains `libc::` only inside a
/// string literal.  The FFI boundary heuristic must not route this as a
/// foreign-libc call because the token appears in code text, not a call site.
pub fn describe_api() -> &'static str {
    // SAFETY: no actual foreign call is made; the string is only data.
    unsafe { "libc::strlen is a C function" }
}

#[cfg(test)]
mod tests {
    use super::describe_api;

    #[test]
    fn mentions_describe_api() {
        let _ = describe_api();
    }
}
