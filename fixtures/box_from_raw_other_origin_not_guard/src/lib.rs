pub fn rebuild_foreign_origin(ptr: *mut u8, value: Box<u8>) -> Box<u8> {
    let _other = Box::into_raw(value);
    // SAFETY: this fixture intentionally creates Box::into_raw evidence for a different pointer.
    unsafe { Box::from_raw(ptr) }
}

#[cfg(test)]
mod tests {
    use super::rebuild_foreign_origin;

    #[test]
    fn mentions_rebuild_foreign_origin() {
        let _ = stringify!(rebuild_foreign_origin);
    }
}
