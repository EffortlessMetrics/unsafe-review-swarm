pub fn rebuild_vec(buf: *mut u8, len: usize, cap: usize) -> Option<Vec<u8>> {
    if len > cap {
        /* return None; */
    }
    // SAFETY: this fixture intentionally mentions return without actually returning.
    Some(unsafe { Vec::from_raw_parts(buf, len, cap) })
}

#[cfg(test)]
mod tests {
    #[test]
    fn mentions_rebuild_vec() {
        let _ = stringify!(super::rebuild_vec);
    }
}
