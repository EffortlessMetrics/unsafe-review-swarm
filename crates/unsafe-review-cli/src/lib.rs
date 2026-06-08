#![forbid(unsafe_code)]
mod command;
mod execute;
mod lsp;
mod parse;

/// A typed failure returned by [`run`].
///
/// The distinction lets callers map policy violations to exit 1 and all other
/// failures (usage errors, I/O errors, internal errors) to exit 2, matching the
/// stable exit-code contract:
///
/// ```text
/// 0 = ran to completion: clean, or advisory findings
/// 1 = ran to completion: no-new-debt policy found new/worsened coverage gaps
/// 2 = tool did not complete a review: usage, input/IO, or internal error
/// ```
pub enum RunFailure {
    /// The tool ran to completion and the no-new-debt policy found new or
    /// worsened coverage gaps.
    PolicyViolation(String),
    /// The tool did not complete a review: usage error, missing/unreadable
    /// input, I/O error, or internal error.
    Tool(String),
}

impl std::fmt::Display for RunFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunFailure::PolicyViolation(msg) | RunFailure::Tool(msg) => f.write_str(msg),
        }
    }
}

/// Converts a plain `String` error into the [`RunFailure::Tool`] variant so
/// that the hundreds of existing `?`-propagated `String` error sites remain
/// unchanged.
impl From<String> for RunFailure {
    fn from(msg: String) -> Self {
        RunFailure::Tool(msg)
    }
}

pub fn run(args: impl IntoIterator<Item = String>) -> Result<(), RunFailure> {
    let command = parse::parse(args).map_err(RunFailure::Tool)?;
    execute::execute(command)
}

#[cfg(test)]
mod tests {
    use super::RunFailure;

    #[test]
    fn string_converts_to_tool_variant() {
        let f: RunFailure = "some tool error".to_string().into();
        assert!(
            matches!(f, RunFailure::Tool(_)),
            "From<String> must map to RunFailure::Tool, not PolicyViolation"
        );
    }

    #[test]
    fn display_shows_inner_message() {
        let tool = RunFailure::Tool("tool message".to_string());
        assert_eq!(tool.to_string(), "tool message");
        let policy = RunFailure::PolicyViolation("policy message".to_string());
        assert_eq!(policy.to_string(), "policy message");
    }
}
