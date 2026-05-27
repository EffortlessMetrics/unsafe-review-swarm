use super::{branch_still_open_at_operation, compact_code};

pub(super) fn has_unreachable_unchecked_infallible_path_evidence(lower: &str) -> bool {
    let compact = compact_code(lower);
    let Some(context) = UnreachableUncheckedPathContext::from_compact(&compact) else {
        return false;
    };
    context.has_open_infallible_match_context()
}

// Infallible-path evidence only applies when the unchecked call is inside the
// same still-open match arm whose head establishes Fallibility::Infallible.
struct UnreachableUncheckedPathContext<'a> {
    before_call: &'a str,
}

impl<'a> UnreachableUncheckedPathContext<'a> {
    fn from_compact(compact: &'a str) -> Option<Self> {
        let call_pos = compact.find("unreachable_unchecked(")?;
        Some(Self {
            before_call: &compact[..call_pos],
        })
    }

    fn has_open_infallible_match_context(self) -> bool {
        let Some(match_context) = self.enclosing_match_context() else {
            return false;
        };
        let Some((match_head, after_open)) = match_context.split_once('{') else {
            return false;
        };
        match_head.contains("fallibility::infallible") && branch_still_open_at_operation(after_open)
    }

    fn enclosing_match_context(self) -> Option<&'a str> {
        let match_pos = self.before_call.rfind("match")?;
        Some(&self.before_call[match_pos..])
    }
}
