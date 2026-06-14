/// Writes `value` to the first element of `buf`.
///
/// The only bounds check here is `debug_assert!`, which is compiled out in
/// release builds.  unsafe-review must not credit it as a runtime guard and
/// must emit a `guard_missing` card.
pub fn write_first(buf: &mut [u32], value: u32) {
    debug_assert!(!buf.is_empty());
    // SAFETY: this fixture intentionally uses only debug_assert! for bounds;
    // no release-runtime guard is present.
    unsafe { buf.as_mut_ptr().write(value) };
}

#[cfg(test)]
mod tests {
    use super::write_first;

    #[test]
    fn writes_to_non_empty_buffer() {
        let mut buf = [0_u32; 4];
        write_first(&mut buf, 42);
    }
}
