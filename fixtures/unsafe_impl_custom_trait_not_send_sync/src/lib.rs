/// # Safety
///
/// Implementors must uphold the local marker contract.
pub unsafe trait LocalContract {}

pub struct Token;

// SAFETY: this fixture states the local unsafe trait contract but is not Send/Sync.
unsafe impl LocalContract for Token {}

#[cfg(test)]
mod tests {
    use super::Token;

    #[test]
    fn constructs_token() {
        let _token = Token;
    }
}
