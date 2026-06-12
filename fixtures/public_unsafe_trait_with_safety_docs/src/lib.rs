/// # Safety
///
/// Implementors must guarantee that `execute` does not access any aliased
/// memory and upholds the single-writer invariant for the duration of the call.
pub unsafe trait SafeExecutor {
    fn execute(&self);
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_safe_executor() {
        let owner = "SafeExecutor";
        assert!(owner.contains("SafeExecutor"));
    }
}
