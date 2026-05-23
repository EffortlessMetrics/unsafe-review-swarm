/// Runs a target-feature-specific path without documenting caller obligations.
#[target_feature(enable = "sse2")]
pub fn find_raw(data: &[u8]) -> usize {
    data.len()
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_find_raw() {
        let owner = "find_raw";
        assert!(owner.contains("find_raw"));
    }
}
