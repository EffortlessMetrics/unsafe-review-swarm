use std::path::Path;
use std::time::{Duration, Instant};

use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, DiffSource, DiscoveryOptions, PolicyMode, RepoScanStatus, Scope,
};

const ENDPOINTS: &[(&str, &str)] = &[
    ("badges/unsafe-review.json", "unsafe-review"),
    ("badges/unsafe-review-plus.json", "unsafe-review+"),
];

const ENDPOINT_PREFIX: &str = "https://img.shields.io/endpoint?url=https%3A%2F%2Fraw.githubusercontent.com%2FEffortlessMetrics%2Funsafe-review%2Fmain%2Fbadges%2F";
const FORBIDDEN_MESSAGE_TERMS: &[&str] = &["safe", "sound", "ub-free", "miri-clean", "proof"];
// This validator regenerates the full repo badge projection.  The bound is a
// hard stop for stale or pathological scans, not a cheap-smoke-test budget.
const GENERATED_PROJECTION_TIMEOUT_SECS: u64 = 900;
const SHIELDS_ENDPOINT_FIELDS: &[&str] = &[
    "schemaVersion",
    "label",
    "message",
    "color",
    "labelColor",
    "isError",
    "namedLogo",
    "logoSvg",
    "style",
    "cacheSeconds",
];

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
    check_generated_projection_with_timeout(Duration::from_secs(
        GENERATED_PROJECTION_TIMEOUT_SECS,
    ))?;
    println!("check-generated-projection: ok");
    Ok(())
}

fn check_generated_projection_with_timeout(timeout: Duration) -> Result<(), String> {
    check_generated_projection_at(&crate::repo_path("."), timeout)
}

fn check_generated_projection_at(root: &Path, timeout: Duration) -> Result<(), String> {
    let started = Instant::now();
    let output = unsafe_review_core::analyze_with_discovery_and_repo_events(
        AnalyzeInput {
            root: root.to_path_buf(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        },
        DiscoveryOptions::repo_defaults(),
        |event| {
            if generated_projection_timeout_elapsed(started, timeout) {
                return Err(generated_projection_timeout_message(timeout, &event.status));
            }
            Ok(())
        },
    )?;
    let (main, plus) = unsafe_review_core::render_badge_jsons(&output);
    for (path, expected_text) in [
        ("badges/unsafe-review.json", main),
        ("badges/unsafe-review-plus.json", plus),
    ] {
        check_generated_endpoint_json(root, path, &expected_text)?;
    }
    Ok(())
}

fn generated_projection_timeout_elapsed(started: Instant, timeout: Duration) -> bool {
    started.elapsed() >= timeout
}

fn generated_projection_timeout_message(timeout: Duration, status: &RepoScanStatus) -> String {
    let timeout_secs = timeout.as_secs();
    let remaining = status.files_discovered.saturating_sub(status.files_scanned);
    let last_path = status
        .last_path
        .as_ref()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| "-".to_string());
    format!(
        "public badge projection exceeded {timeout_secs}s while {phase}; \
         files_discovered={files_discovered} files_scanned={files_scanned} \
         files_remaining={remaining} cards_found={cards_found} last_path={last_path}; \
         badge staleness was not revalidated. Run \
         `unsafe-review repo --format json --out target/unsafe-review-badge-projection.json \
         --progress --timeout-seconds {timeout_secs}` to diagnose, then run \
         `unsafe-review badges --out badges/` after the full repo projection completes.",
        phase = status.phase.as_str(),
        files_discovered = status.files_discovered,
        files_scanned = status.files_scanned,
        cards_found = status.cards_found,
    )
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
    reject_non_shields_endpoint_fields(path, &value)?;
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

fn reject_non_shields_endpoint_fields(path: &str, value: &serde_json::Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{path} badge endpoint payload must be a JSON object"))?;
    for key in object.keys() {
        if !SHIELDS_ENDPOINT_FIELDS.contains(&key.as_str()) {
            return Err(format!(
                "{path} badge endpoint payload contains non-Shields field `{key}`"
            ));
        }
    }
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

fn check_generated_endpoint_json(
    root: &Path,
    path: &str,
    expected_text: &str,
) -> Result<(), String> {
    let actual = crate::parse_json_file(&root.join(path))?;
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

// Unit consumers use the real analyzer/renderer/comparator on a tiny repository.
// The mandatory public-surface producer above still scans the current workspace.
#[cfg(test)]
pub(crate) fn check_fixture_projection() -> Result<(), String> {
    tests::fixture_projection_contract()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture(PathBuf);

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    pub(super) fn fixture_projection_contract() -> Result<(), String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("badge-projection-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).map_err(|err| err.to_string())?;
        let fixture = Fixture(root);
        let root = &fixture.0;
        fs::create_dir(root.join("badges")).map_err(|err| err.to_string())?;
        fs::write(root.join("lib.rs"), "pub fn value() -> u8 { 0 }\n")
            .map_err(|err| err.to_string())?;
        for (path, label) in ENDPOINTS {
            write_zero_endpoint(&root.join(path), label)?;
        }
        let check = || check_generated_projection_at(root, Duration::from_secs(10));
        check()?;

        // Independently corrupt each endpoint; full JSON equality also protects
        // non-count fields. Invalid/missing files must not become cached success.
        for (path, label) in ENDPOINTS {
            let endpoint = root.join(path);
            for (field, value) in [("message", "1"), ("color", "orange")] {
                let mut badge = crate::parse_json_file(&endpoint)?;
                badge[field] = serde_json::json!(value);
                fs::write(&endpoint, badge.to_string()).map_err(|err| err.to_string())?;
                let err = check().expect_err("stale endpoint must fail");
                assert!(
                    err.contains(path.rsplit('/').next().unwrap_or(path)),
                    "{err}"
                );
                assert!(err.contains("is stale"), "{err}");
                write_zero_endpoint(&endpoint, label)?;
            }
            fs::write(&endpoint, "{").map_err(|err| err.to_string())?;
            assert!(check().is_err(), "malformed endpoint must fail");
            fs::remove_file(&endpoint).map_err(|err| err.to_string())?;
            assert!(check().is_err(), "missing endpoint must fail");
            write_zero_endpoint(&endpoint, label)?;
        }
        check()?;

        fs::write(
            root.join("lib.rs"),
            "pub unsafe fn load(p: *const u8) -> u8 { unsafe { *p } }\n",
        )
        .map_err(|err| err.to_string())?;
        let err = check().expect_err("changed source must invalidate old badges");
        assert!(err.contains("is stale"), "{err}");
        assert!(err.contains("checked-in message 0"), "{err}");
        assert!(!err.contains("generated message 0"), "{err}");
        fs::write(root.join("lib.rs"), "pub fn value() -> u8 { 0 }\n")
            .map_err(|err| err.to_string())?;
        check()?;

        let err = check_generated_projection_at(root, Duration::ZERO)
            .expect_err("timeout must propagate instead of accepting old badges");
        assert!(err.contains("exceeded 0s"), "{err}");
        Ok(())
    }

    fn write_zero_endpoint(path: &Path, label: &str) -> Result<(), String> {
        let badge = serde_json::json!({
            "schemaVersion": 1,
            "label": label,
            "message": "0",
            "color": "green"
        });
        fs::write(path, badge.to_string()).map_err(|err| err.to_string())
    }

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
        let main_err = match require_numeric_message("badges/unsafe-review.json", "294 open gaps") {
            Ok(()) => return Err("descriptive badge message should be rejected".to_string()),
            Err(err) => err,
        };
        assert!(main_err.contains("numeric count"));
        let plus_err = match require_numeric_message(
            "badges/unsafe-review-plus.json",
            "19 contract / 111 guard / 37 witness",
        ) {
            Ok(()) => return Err("compound badge message should be rejected".to_string()),
            Err(err) => err,
        };
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
    fn endpoint_json_rejects_internal_contract_fields() {
        let value = serde_json::json!({
            "schemaVersion": 1,
            "contract_version": "0.1",
            "kind": "unsafe_review",
            "scope": "repo",
            "basis": "open_actionable_review_gaps",
            "label": "unsafe-review",
            "message": "7",
            "status": "fail",
            "color": "orange",
            "counts": {
                "unsuppressed_review_gaps": 7
            }
        });

        let err = reject_non_shields_endpoint_fields("badges/unsafe-review.json", &value)
            .err()
            .unwrap_or_default();

        assert!(err.contains("non-Shields field"));
        assert!(
            [
                "contract_version",
                "kind",
                "scope",
                "basis",
                "status",
                "counts"
            ]
            .iter()
            .any(|field| err.contains(field))
        );
    }

    #[test]
    fn generated_staleness_message_names_actual_and_expected_counts() {
        let actual = serde_json::json!({ "message": "1" });
        let expected = serde_json::json!({ "message": "2" });

        let message = stale_generated_message("badges/unsafe-review.json", &actual, &expected);

        assert!(message.contains("checked-in message 1"));
        assert!(message.contains("generated message 2"));
    }

    #[test]
    fn generated_projection_timeout_message_reports_progress() {
        let status = RepoScanStatus {
            schema_version: "repo-scan-status/v1".to_string(),
            phase: unsafe_review_core::RepoScanPhase::Scanning,
            elapsed_ms: 7_001,
            files_discovered: 42,
            files_scanned: 12,
            cards_found: 3,
            last_path: Some("fixtures/slow/src/lib.rs".into()),
            completed: false,
            partial: false,
            stop_reason: unsafe_review_core::RepoStopReason::None,
            cap: None,
            file_timings: None,
            output_bytes: None,
        };

        let message = generated_projection_timeout_message(Duration::from_secs(7), &status);

        assert!(message.contains("exceeded 7s"));
        assert!(message.contains("while scanning"));
        assert!(message.contains("files_discovered=42"));
        assert!(message.contains("files_scanned=12"));
        assert!(message.contains("files_remaining=30"));
        assert!(message.contains("cards_found=3"));
        assert!(message.contains("last_path=fixtures/slow/src/lib.rs"));
        assert!(message.contains("unsafe-review repo --format json"));
        assert!(message.contains("unsafe-review badges --out badges/"));
    }

    #[test]
    fn generated_projection_timeout_elapsed_trips_after_deadline() {
        let started = Instant::now() - Duration::from_secs(8);

        assert!(generated_projection_timeout_elapsed(
            started,
            Duration::from_secs(7)
        ));
    }
}
