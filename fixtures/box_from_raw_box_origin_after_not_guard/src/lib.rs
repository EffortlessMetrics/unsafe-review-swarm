pub fn rebuild_then_create_origin(ptr: *mut u8, value: Box<u8>) -> Box<u8> {
    // SAFETY: this fixture intentionally creates Box::into_raw evidence only after from_raw.
    let rebuilt = unsafe { Box::from_raw(ptr) };
    let _later = Box::into_raw(value);
    rebuilt
}

#[cfg(test)]
mod tests {
    use super::rebuild_then_create_origin;

    #[test]
    fn mentions_rebuild_then_create_origin() {
        let _ = stringify!(rebuild_then_create_origin);
    }
}
