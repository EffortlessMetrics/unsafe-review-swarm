use super::syntax_site_uses_exact_range;
use crate::domain::UnsafeSiteKind;
use crate::input::diff::DiffIndex;
use std::path::PathBuf;

pub(super) fn site_changed(
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    rel: &PathBuf,
    line: usize,
    end_line: usize,
    kind: &UnsafeSiteKind,
) -> bool {
    diff.is_none_or(|d| {
        repo_mode
            || if syntax_site_uses_exact_range(kind) {
                d.contains_in_range(rel, line, end_line)
            } else {
                d.contains_near(rel, line)
            }
    })
}
