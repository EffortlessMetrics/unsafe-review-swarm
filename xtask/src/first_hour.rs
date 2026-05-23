use std::path::Path;

use crate::{markdown, read_to_string, require_file, text_contains_ignore_ascii_case};

const FIRST_HOUR_DOC: &str = "docs/FIRST_HOUR.md";
const FIRST_USE_DOC: &str = "docs/FIRST_USE.md";
const ROOT_README: &str = "README.md";
const DOCS_MAP: &str = "docs/README.md";

const REQUIRED_COMMANDS: &[&str] = &[
    "cargo install unsafe-review --locked",
    "unsafe-review doctor",
    "unsafe-review first-pr --base origin/main",
    "unsafe-review explain <card-id>",
    "unsafe-review support",
];

const REQUIRED_ARTIFACT_PATHS: &[&str] = &[
    "target/unsafe-review/pr-summary.md",
    "target/unsafe-review/cards.json",
    "target/unsafe-review/cards.sarif",
    "target/unsafe-review/comment-plan.json",
    "target/unsafe-review/witness-plan.md",
    "target/unsafe-review/lsp.json",
];

const REQUIRED_FIXTURE_PATH: &str = "fixtures/raw_pointer_alignment";

const REQUIRED_TRUST_BOUNDARY_PHRASES: &[&str] =
    &["does not prove memory safety", "does not", "advisory"];

const REQUIRED_NEGATIVE_BOUNDARY_TOPICS: &[&str] = &[
    "miri", "ub-free", "blocking", "witness", "comments", "source",
];

const FORBIDDEN_PHRASES: &[&str] = &[
    "unsafe-review proves",
    "guaranteed safe",
    "guaranteed UB-free",
    "is proof of",
    "is a proof of",
];

pub(crate) fn check_first_hour() -> Result<(), String> {
    require_file(FIRST_HOUR_DOC)?;

    let doc_path = Path::new(FIRST_HOUR_DOC);
    let text = read_to_string(doc_path)?;

    require_commands_present(&text)?;
    require_artifact_paths_present(&text)?;
    require_fixture_reference(&text)?;
    require_trust_boundary_present(&text)?;
    require_no_overclaims(&text)?;
    require_inbound_links()?;
    require_docs_map_entry()?;

    println!("check-first-hour: ok ({FIRST_HOUR_DOC})");
    Ok(())
}

fn require_commands_present(text: &str) -> Result<(), String> {
    for needle in REQUIRED_COMMANDS {
        if !text.contains(needle) {
            return Err(format!(
                "{FIRST_HOUR_DOC} is missing required first-hour command `{needle}`"
            ));
        }
    }
    Ok(())
}

fn require_artifact_paths_present(text: &str) -> Result<(), String> {
    for needle in REQUIRED_ARTIFACT_PATHS {
        if !text.contains(needle) {
            return Err(format!(
                "{FIRST_HOUR_DOC} is missing first-pr bundle path `{needle}`"
            ));
        }
    }
    Ok(())
}

fn require_fixture_reference(text: &str) -> Result<(), String> {
    if !text.contains(REQUIRED_FIXTURE_PATH) {
        return Err(format!(
            "{FIRST_HOUR_DOC} must reference the deterministic fixture `{REQUIRED_FIXTURE_PATH}`"
        ));
    }
    if !Path::new(REQUIRED_FIXTURE_PATH).is_dir() {
        return Err(format!(
            "first-hour fixture directory missing: {REQUIRED_FIXTURE_PATH}"
        ));
    }
    Ok(())
}

fn require_trust_boundary_present(text: &str) -> Result<(), String> {
    for needle in REQUIRED_TRUST_BOUNDARY_PHRASES {
        if !text_contains_ignore_ascii_case(text, needle) {
            return Err(format!(
                "{FIRST_HOUR_DOC} trust boundary is missing `{needle}`"
            ));
        }
    }
    for topic in REQUIRED_NEGATIVE_BOUNDARY_TOPICS {
        if !text_contains_ignore_ascii_case(text, topic) {
            return Err(format!(
                "{FIRST_HOUR_DOC} non-goals are missing trust-boundary topic `{topic}`"
            ));
        }
    }
    Ok(())
}

fn require_no_overclaims(text: &str) -> Result<(), String> {
    let lower = text.to_ascii_lowercase();
    for needle in FORBIDDEN_PHRASES {
        let pattern = needle.to_ascii_lowercase();
        for (offset, _) in lower.match_indices(&pattern) {
            if is_negated_use(&lower, offset, pattern.len()) {
                continue;
            }
            return Err(format!(
                "{FIRST_HOUR_DOC} contains forbidden overclaim `{needle}`"
            ));
        }
    }
    Ok(())
}

fn is_negated_use(lower: &str, offset: usize, len: usize) -> bool {
    let mut window_start = offset.saturating_sub(80);
    while window_start > 0 && !lower.is_char_boundary(window_start) {
        window_start -= 1;
    }
    let context_before = &lower[window_start..offset];
    let after_offset = offset + len;
    let mut context_after_end = (after_offset + 40).min(lower.len());
    while context_after_end < lower.len() && !lower.is_char_boundary(context_after_end) {
        context_after_end += 1;
    }
    let context_after = &lower[after_offset..context_after_end];

    let negative_markers = [
        "not ", "no ", "does not", "doesn't", "without", "never", "cannot", "can't", "isn't",
        "is not",
    ];
    if negative_markers
        .iter()
        .any(|marker| context_before.contains(marker))
    {
        return true;
    }
    if context_after.starts_with(" claim") || context_after.starts_with(" status") {
        if context_before.contains("no ")
            || context_before.contains("not")
            || context_before.contains("without")
        {
            return true;
        }
    }
    false
}

fn require_inbound_links() -> Result<(), String> {
    require_link_to_first_hour(Path::new(ROOT_README))?;
    require_link_to_first_hour(Path::new(FIRST_USE_DOC))?;
    Ok(())
}

fn require_link_to_first_hour(source: &Path) -> Result<(), String> {
    let text = read_to_string(source)?;
    let want_targets = ["docs/FIRST_HOUR.md", "FIRST_HOUR.md"];
    for target in markdown::link_targets(&text) {
        if want_targets.iter().any(|want| target.ends_with(want)) {
            return Ok(());
        }
    }
    Err(format!(
        "{} must link to {FIRST_HOUR_DOC}",
        source.display()
    ))
}

fn require_docs_map_entry() -> Result<(), String> {
    let text = read_to_string(Path::new(DOCS_MAP))?;
    let mut saw = false;
    for span in markdown::code_spans(&text) {
        if span.trim() == FIRST_HOUR_DOC {
            saw = true;
            break;
        }
    }
    if !saw {
        return Err(format!(
            "{DOCS_MAP} must list `{FIRST_HOUR_DOC}` in its docs map code spans"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overclaim_detection_allows_negated_uses() -> Result<(), String> {
        let text = "It does not prove memory safety. It is not Miri-clean.";
        require_no_overclaims(text)
    }

    #[test]
    fn overclaim_detection_catches_positive_uses() {
        let text = "This is guaranteed safe.";
        let result = require_no_overclaims(text);
        assert!(result.is_err(), "expected overclaim error, got {result:?}");
    }

    #[test]
    fn overclaim_detection_handles_non_ascii_context() -> Result<(), String> {
        let text = "Step 1 — install. This does not mean unsafe-review proves memory safety.";
        require_no_overclaims(text)
    }
}
