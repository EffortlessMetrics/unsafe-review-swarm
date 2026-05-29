use unsafe_review_core::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope};

const ENDPOINTS: &[(&str, &str)] = &[
    ("badges/unsafe-review.json", "unsafe-review"),
    ("badges/unsafe-review-plus.json", "unsafe-review+"),
];

const ENDPOINT_PREFIX: &str = "https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FEffortlessMetrics%2Funsafe-review%2Fmain%2Fbadges%2F";
const FORBIDDEN_MESSAGE_TERMS: &[&str] = &["safe", "sound", "ub-free", "miri-clean", "proof"];

pub(crate) fn endpoint_count() -> usize {
    ENDPOINTS.len()
}

pub(crate) fn check_endpoints() -> Result<(), String> {
    let readme = crate::read_to_string(&crate::workspace_path("README.md"))?;
    let endpoint_links = readme.matches(ENDPOINT_PREFIX).count();
    if endpoint_links != ENDPOINTS.len() {
        return Err(format!(
            "README.md has {endpoint_links} public unsafe-review badge endpoint link(s), expected {}",
            ENDPOINTS.len()
        ));
    }

    for (path, label) in ENDPOINTS {
        check_readme_endpoint(&readme, path)?;
        check_endpoint_json(path, label)?;
    }
    Ok(())
}

pub(crate) fn check_generated_projection() -> Result<(), String> {
    let output = unsafe_review_core::analyze(AnalyzeInput {
        root: crate::repo_path("."),
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Repo,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: None,
    })?;
    let (main, plus) = unsafe_review_core::render_badge_jsons(&output);
    for (path, expected_text) in [
        ("badges/unsafe-review.json", main),
        ("badges/unsafe-review-plus.json", plus),
    ] {
        check_generated_endpoint_json(path, &expected_text)?;
    }
    Ok(())
}

pub(crate) fn is_public_endpoint(path: &str) -> bool {
    ENDPOINTS.iter().any(|(endpoint, _label)| *endpoint == path)
}

pub(crate) fn require_numeric_message(path: &str, message: &str) -> Result<(), String> {
    if message.chars().all(|ch| ch.is_ascii_digit()) {
        Ok(())
    } else {
        Err(format!(
            "{path} badge message must be a numeric count; got `{message}`"
        ))
    }
}

pub(crate) fn endpoint_url(path: &str) -> String {
    let encoded_path = path.replace('/', "%2F");
    format!(
        "https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FEffortlessMetrics%2Funsafe-review%2Fmain%2F{encoded_path}"
    )
}

fn check_readme_endpoint(readme: &str, path: &str) -> Result<(), String> {
    let endpoint = endpoint_url(path);
    if readme.contains(&endpoint) {
        Ok(())
    } else {
        Err(format!(
            "README.md is missing public badge endpoint `{endpoint}`"
        ))
    }
}

fn check_endpoint_json(path: &str, label: &str) -> Result<(), String> {
    let value = crate::parse_json_file(&crate::workspace_path(path))?;
    let schema = crate::json_usize_at(&value, "/schemaVersion", path)?;
    if schema != 1 {
        return Err(format!("{path} schemaVersion is {schema}, expected 1"));
    }
    crate::require_json_str(&value, "label", label, path)?;
    let message = crate::require_non_empty_json_str(&value, "message", path)?;
    require_numeric_message(path, message)?;
    reject_forbidden_message_terms(path, message)?;
    crate::require_non_empty_json_str(&value, "color", path)?;
    Ok(())
}

fn reject_forbidden_message_terms(path: &str, message: &str) -> Result<(), String> {
    for forbidden in FORBIDDEN_MESSAGE_TERMS {
        if crate::text_contains_ignore_ascii_case(message, forbidden) {
            return Err(format!(
                "{path} badge message must not imply `{forbidden}`: {message}"
            ));
        }
    }
    Ok(())
}

fn check_generated_endpoint_json(path: &str, expected_text: &str) -> Result<(), String> {
    let actual = crate::parse_json_file(&crate::workspace_path(path))?;
    let expected = generated_json(path, expected_text)?;
    if actual == expected {
        Ok(())
    } else {
        Err(stale_generated_message(path, &actual, &expected))
    }
}

fn generated_json(path: &str, text: &str) -> Result<serde_json::Value, String> {
    serde_json::from_str(text)
        .map_err(|err| format!("generated badge JSON for {path} did not parse: {err}"))
}

fn stale_generated_message(
    path: &str,
    actual: &serde_json::Value,
    expected: &serde_json::Value,
) -> String {
    let actual_message = message_or_missing(actual);
    let expected_message = message_or_missing(expected);
    format!(
        "{path} is stale (checked-in message {actual_message}, generated message {expected_message}); run `cargo run --locked -p unsafe-review -- badges --out badges/`"
    )
}

fn message_or_missing(value: &serde_json::Value) -> &str {
    value
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("<missing>")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_url_uses_public_source_repo() {
        assert_eq!(
            endpoint_url("badges/unsafe-review.json"),
            "https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FEffortlessMetrics%2Funsafe-review%2Fmain%2Fbadges%2Funsafe-review.json"
        );
    }

    #[test]
    fn numeric_messages_are_counts_only() -> Result<(), String> {
        require_numeric_message("badges/unsafe-review.json", "294")?;
        let main_err = require_numeric_message("badges/unsafe-review.json", "294 open gaps")
            .expect_err("descriptive badge message should be rejected");
        assert!(main_err.contains("numeric count"));
        let plus_err = require_numeric_message(
            "badges/unsafe-review-plus.json",
            "19 contract / 111 guard / 37 witness",
        )
        .expect_err("compound badge message should be rejected");
        assert!(plus_err.contains("numeric count"));
        Ok(())
    }

    #[test]
    fn endpoint_allowlist_distinguishes_generated_badges() {
        assert!(is_public_endpoint("badges/unsafe-review.json"));
        assert!(is_public_endpoint("badges/unsafe-review-plus.json"));
        assert!(!is_public_endpoint("badges/local-only.json"));
    }

    #[test]
    fn generated_staleness_message_names_actual_and_expected_counts() {
        let actual = serde_json::json!({ "message": "1" });
        let expected = serde_json::json!({ "message": "2" });

        let message = stale_generated_message("badges/unsafe-review.json", &actual, &expected);

        assert!(message.contains("checked-in message 1"));
        assert!(message.contains("generated message 2"));
    }
}
