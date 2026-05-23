use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

const LABEL_DIR: &str = "docs/accuracy/labels";
const LABEL_LEDGER_FIELDS: &[&str] = &[
    "schema_version",
    "status",
    "claim_id",
    "operation_family",
    "hazard",
    "partition",
    "source_kind",
    "trust_boundary",
    "samples",
];
const LABEL_SAMPLE_FIELDS: &[&str] = &[
    "id",
    "fixture",
    "kind",
    "expected_cards",
    "expected_class",
    "expected_operation_family",
    "expected_hazard",
    "expected_obligation_key",
    "expected_contract_state",
    "expected_discharge_state",
    "expected_witness_route_kinds",
    "label_source",
    "labelers",
    "adjudicator",
    "rationale",
];
const LABEL_LEDGER_STATUSES: &[&str] =
    &["fixture_pinned", "dogfood_measured", "labeled_calibrated"];
const LABEL_PARTITIONS: &[&str] = &["fixture", "dogfood", "labeled", "holdout"];
const LABEL_SOURCE_KINDS: &[&str] = &["fixture_golden", "human_adjudicated"];
const LABEL_SAMPLE_KINDS: &[&str] = &["positive", "negative", "false_positive_control"];
const DISCHARGE_STATES: &[&str] = &["present", "missing"];
const WITNESS_ROUTE_KINDS: &[&str] = &[
    "miri",
    "cargo-careful",
    "asan",
    "msan",
    "tsan",
    "lsan",
    "loom",
    "shuttle",
    "kani",
    "crux",
    "human-deep-review",
];

#[derive(Clone, Debug)]
pub(crate) struct CalibrationFixtureCase {
    pub(crate) kind: String,
    pub(crate) expected_cards: usize,
    pub(crate) expected_class: Option<String>,
    pub(crate) expected_operation_family: Option<String>,
    pub(crate) expected_hazard: Option<String>,
}

#[derive(Debug)]
struct PolicyClaim {
    operation_family: Option<String>,
    hazard: Option<String>,
    label_ledgers: BTreeSet<String>,
}

struct FixtureObligation<'a> {
    fixture: &'a str,
    operation_family: &'a str,
    hazard: &'a str,
    obligation_key: &'a str,
}

struct EvidenceStateExpectation<'a> {
    evidence_kind: &'a str,
    field: &'a str,
    expected_state: &'a str,
}

pub(crate) fn check_accuracy_label_ledgers(
    policy: &toml::Value,
    fixture_cases: &BTreeMap<String, CalibrationFixtureCase>,
) -> Result<usize, String> {
    let claims = policy_claims(policy)?;
    let referenced_ledgers = claims
        .values()
        .flat_map(|claim| claim.label_ledgers.iter().cloned())
        .collect::<BTreeSet<_>>();
    let actual_ledgers = label_ledger_files()?;
    if actual_ledgers.is_empty() {
        return Err(format!(
            "{LABEL_DIR} must contain at least one calibration label ledger"
        ));
    }
    for ledger in &actual_ledgers {
        if !referenced_ledgers.contains(ledger) {
            return Err(format!(
                "{ledger} exists but is not referenced by policy/accuracy-calibration.toml label_ledgers"
            ));
        }
    }

    let mut sample_count = 0usize;
    for (claim_id, claim) in &claims {
        for ledger in &claim.label_ledgers {
            if !actual_ledgers.contains(ledger) {
                return Err(format!(
                    "policy/accuracy-calibration.toml claim `{claim_id}` references missing label ledger `{ledger}`"
                ));
            }
            let value = super::parse_toml_file(&super::workspace_path(ledger))?;
            sample_count += validate_label_ledger(ledger, &value, claim_id, claim, fixture_cases)?;
        }
    }

    Ok(sample_count)
}

fn policy_claims(policy: &toml::Value) -> Result<BTreeMap<String, PolicyClaim>, String> {
    let claims = super::toml_array(policy, "claim", "policy/accuracy-calibration.toml")?;
    let mut result = BTreeMap::new();
    let mut ledger_owners = BTreeMap::new();
    for (idx, claim) in claims.iter().enumerate() {
        let claim = claim.as_table().ok_or_else(|| {
            format!("policy/accuracy-calibration.toml claim[{idx}] must be a TOML table")
        })?;
        let id = required_table_string(claim, "id", "policy/accuracy-calibration.toml", idx)?;
        let operation_family = optional_table_string(
            claim,
            "operation_family",
            "policy/accuracy-calibration.toml",
            idx,
        )?
        .map(str::to_string);
        let hazard =
            optional_table_string(claim, "hazard", "policy/accuracy-calibration.toml", idx)?
                .map(str::to_string);
        let label_ledgers = optional_table_str_array(
            claim,
            "label_ledgers",
            "policy/accuracy-calibration.toml",
            idx,
        )?;
        let mut ledgers = BTreeSet::new();
        for ledger in label_ledgers {
            validate_label_ledger_path(ledger, "policy/accuracy-calibration.toml", idx)?;
            if !ledgers.insert(ledger.to_string()) {
                return Err(format!(
                    "policy/accuracy-calibration.toml claim[{idx}] duplicates label ledger `{ledger}`"
                ));
            }
            if let Some(previous) = ledger_owners.insert(ledger.to_string(), id.to_string()) {
                return Err(format!(
                    "policy/accuracy-calibration.toml label ledger `{ledger}` is referenced by both `{previous}` and `{id}`"
                ));
            }
        }
        if result
            .insert(
                id.to_string(),
                PolicyClaim {
                    operation_family,
                    hazard,
                    label_ledgers: ledgers,
                },
            )
            .is_some()
        {
            return Err(format!(
                "policy/accuracy-calibration.toml contains duplicate claim id `{id}`"
            ));
        }
    }
    Ok(result)
}

fn validate_label_ledger(
    path: &str,
    value: &toml::Value,
    claim_id: &str,
    claim: &PolicyClaim,
    fixture_cases: &BTreeMap<String, CalibrationFixtureCase>,
) -> Result<usize, String> {
    let table = value
        .as_table()
        .ok_or_else(|| format!("{path} must contain a TOML table"))?;
    for field in table.keys() {
        if !LABEL_LEDGER_FIELDS.contains(&field.as_str()) {
            return Err(format!("{path} uses unknown field `{field}`"));
        }
    }
    require_exact_string(table, "schema_version", "0.1", path, 0)?;
    let status = required_table_string(table, "status", path, 0)?;
    require_allowed(status, LABEL_LEDGER_STATUSES, path, "status")?;
    require_exact_string(table, "claim_id", claim_id, path, 0)?;
    if let Some(operation_family) = &claim.operation_family {
        require_exact_string(table, "operation_family", operation_family, path, 0)?;
    } else {
        required_table_string(table, "operation_family", path, 0)?;
    }
    if let Some(hazard) = &claim.hazard {
        require_exact_string(table, "hazard", hazard, path, 0)?;
    } else {
        required_table_string(table, "hazard", path, 0)?;
    }
    let partition = required_table_string(table, "partition", path, 0)?;
    require_allowed(partition, LABEL_PARTITIONS, path, "partition")?;
    let source_kind = required_table_string(table, "source_kind", path, 0)?;
    require_allowed(source_kind, LABEL_SOURCE_KINDS, path, "source_kind")?;
    let boundary = required_table_string(table, "trust_boundary", path, 0)?;
    super::require_boundary_text(boundary, path)?;
    let samples = super::toml_array(value, "samples", path)?;
    if samples.is_empty() {
        return Err(format!("{path} must contain at least one sample"));
    }
    let mut ids = BTreeSet::new();
    for (idx, sample) in samples.iter().enumerate() {
        let sample = sample
            .as_table()
            .ok_or_else(|| format!("{path} samples[{idx}] must be a TOML table"))?;
        validate_sample(path, idx, sample, source_kind, fixture_cases, &mut ids)?;
    }
    Ok(samples.len())
}

fn validate_sample(
    path: &str,
    idx: usize,
    sample: &toml::map::Map<String, toml::Value>,
    ledger_source_kind: &str,
    fixture_cases: &BTreeMap<String, CalibrationFixtureCase>,
    ids: &mut BTreeSet<String>,
) -> Result<(), String> {
    for field in sample.keys() {
        if !LABEL_SAMPLE_FIELDS.contains(&field.as_str()) {
            return Err(format!(
                "{path} samples[{idx}] uses unknown field `{field}`"
            ));
        }
    }
    let id = required_table_string(sample, "id", path, idx)?;
    if !ids.insert(id.to_string()) {
        return Err(format!("{path} contains duplicate sample id `{id}`"));
    }
    let label_source = required_table_string(sample, "label_source", path, idx)?;
    require_allowed(label_source, LABEL_SOURCE_KINDS, path, "label_source")?;
    if label_source != ledger_source_kind {
        return Err(format!(
            "{path} samples[{idx}] label_source `{label_source}` does not match ledger source_kind `{ledger_source_kind}`"
        ));
    }
    if label_source == "human_adjudicated" {
        let labelers = required_table_str_array(sample, "labelers", path, idx)?;
        if labelers.len() < 2 {
            return Err(format!(
                "{path} samples[{idx}] human_adjudicated samples require at least two labelers"
            ));
        }
        required_table_string(sample, "adjudicator", path, idx)?;
    }
    let fixture = required_table_string(sample, "fixture", path, idx)?;
    let fixture_case = fixture_cases.get(fixture).ok_or_else(|| {
        format!("{path} samples[{idx}] references fixture `{fixture}` not present in fixtures/calibration.toml")
    })?;
    let kind = required_table_string(sample, "kind", path, idx)?;
    require_allowed(kind, LABEL_SAMPLE_KINDS, path, "kind")?;
    if kind != fixture_case.kind {
        return Err(format!(
            "{path} samples[{idx}] kind `{kind}` does not match fixtures/calibration.toml kind `{}` for `{fixture}`",
            fixture_case.kind
        ));
    }
    let expected_cards = required_table_usize(sample, "expected_cards", path, idx)?;
    if expected_cards != fixture_case.expected_cards {
        return Err(format!(
            "{path} samples[{idx}] expected_cards is {expected_cards}, but fixtures/calibration.toml expects {} for `{fixture}`",
            fixture_case.expected_cards
        ));
    }
    let rationale = required_table_string(sample, "rationale", path, idx)?;
    if rationale.len() < 24 {
        return Err(format!("{path} samples[{idx}] rationale is too terse"));
    }

    if expected_cards == 0 {
        reject_zero_card_fields(path, idx, sample)?;
        return Ok(());
    }

    compare_optional_sample_field(
        path,
        idx,
        sample,
        "expected_class",
        fixture,
        fixture_case.expected_class.as_deref(),
    )?;
    let expected_operation_family =
        required_table_string(sample, "expected_operation_family", path, idx)?;
    compare_optional_sample_field(
        path,
        idx,
        sample,
        "expected_operation_family",
        fixture,
        fixture_case.expected_operation_family.as_deref(),
    )?;
    let expected_hazard = required_table_string(sample, "expected_hazard", path, idx)?;
    compare_optional_sample_field(
        path,
        idx,
        sample,
        "expected_hazard",
        fixture,
        fixture_case.expected_hazard.as_deref(),
    )?;
    let obligation_key = required_table_string(sample, "expected_obligation_key", path, idx)?;
    let fixture_obligation = FixtureObligation {
        fixture,
        operation_family: expected_operation_family,
        hazard: expected_hazard,
        obligation_key,
    };
    if let Some(contract_state) =
        optional_table_string(sample, "expected_contract_state", path, idx)?
    {
        require_allowed(
            contract_state,
            DISCHARGE_STATES,
            path,
            "expected_contract_state",
        )?;
        check_fixture_obligation_evidence_state(
            path,
            idx,
            &fixture_obligation,
            EvidenceStateExpectation {
                evidence_kind: "contract",
                field: "expected_contract_state",
                expected_state: contract_state,
            },
        )?;
    }
    let discharge_state = required_table_string(sample, "expected_discharge_state", path, idx)?;
    require_allowed(
        discharge_state,
        DISCHARGE_STATES,
        path,
        "expected_discharge_state",
    )?;
    check_fixture_obligation_evidence_state(
        path,
        idx,
        &fixture_obligation,
        EvidenceStateExpectation {
            evidence_kind: "discharge",
            field: "expected_discharge_state",
            expected_state: discharge_state,
        },
    )?;
    let route_kinds = optional_table_str_array(sample, "expected_witness_route_kinds", path, idx)?;
    if !route_kinds.is_empty() {
        for route_kind in route_kinds {
            require_allowed(
                route_kind,
                WITNESS_ROUTE_KINDS,
                path,
                "expected_witness_route_kinds",
            )?;
            check_fixture_witness_route_kind(
                path,
                idx,
                fixture,
                expected_operation_family,
                expected_hazard,
                route_kind,
            )?;
        }
    }
    Ok(())
}

fn reject_zero_card_fields(
    path: &str,
    idx: usize,
    sample: &toml::map::Map<String, toml::Value>,
) -> Result<(), String> {
    for field in [
        "expected_class",
        "expected_operation_family",
        "expected_hazard",
        "expected_obligation_key",
        "expected_contract_state",
        "expected_discharge_state",
        "expected_witness_route_kinds",
    ] {
        if sample.contains_key(field) {
            return Err(format!(
                "{path} samples[{idx}] zero-card sample must not set `{field}`"
            ));
        }
    }
    Ok(())
}

fn compare_optional_sample_field(
    path: &str,
    idx: usize,
    sample: &toml::map::Map<String, toml::Value>,
    field: &str,
    fixture: &str,
    expected: Option<&str>,
) -> Result<(), String> {
    let actual = required_table_string(sample, field, path, idx)?;
    let Some(expected) = expected else {
        return Err(format!(
            "{path} samples[{idx}] sets `{field}`, but fixtures/calibration.toml has no `{field}` for `{fixture}`"
        ));
    };
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{path} samples[{idx}] `{field}` is `{actual}`, but fixtures/calibration.toml expects `{expected}` for `{fixture}`"
        ))
    }
}

fn check_fixture_obligation_evidence_state(
    path: &str,
    idx: usize,
    obligation: &FixtureObligation<'_>,
    expectation: EvidenceStateExpectation<'_>,
) -> Result<(), String> {
    let cards_path = super::workspace_path("fixtures")
        .join(obligation.fixture)
        .join("expected.cards.json");
    let cards = super::parse_json_file(&cards_path)?;
    let cards = cards.as_array().ok_or_else(|| {
        format!(
            "{}/expected.cards.json must contain a JSON array",
            obligation.fixture
        )
    })?;
    let mut saw_key = false;
    for card in cards {
        if card
            .get("operation_family")
            .and_then(serde_json::Value::as_str)
            != Some(obligation.operation_family)
            || !json_array_contains_str(card, "hazards", obligation.hazard)
        {
            continue;
        }
        for evidence in super::json_array_at(
            card,
            "/obligation_evidence",
            &format!("{}/expected.cards.json", obligation.fixture),
        )? {
            if evidence.get("key").and_then(serde_json::Value::as_str)
                != Some(obligation.obligation_key)
            {
                continue;
            }
            saw_key = true;
            let state_pointer = format!("/{}/state", expectation.evidence_kind);
            let actual = evidence
                .pointer(&state_pointer)
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| {
                    format!(
                        "{}/expected.cards.json obligation `{}` is missing {}.state",
                        obligation.fixture, obligation.obligation_key, expectation.evidence_kind
                    )
                })?;
            if actual == expectation.expected_state {
                return Ok(());
            }
        }
    }
    if saw_key {
        Err(format!(
            "{path} samples[{idx}] {} `{}` was not found for `{}` / `{}` obligation `{}` in `{}`",
            expectation.field,
            expectation.expected_state,
            obligation.operation_family,
            obligation.hazard,
            obligation.obligation_key,
            obligation.fixture
        ))
    } else {
        Err(format!(
            "{path} samples[{idx}] expected_obligation_key `{}` was not found for `{}` / `{}` in `{}`",
            obligation.obligation_key,
            obligation.operation_family,
            obligation.hazard,
            obligation.fixture
        ))
    }
}

fn check_fixture_witness_route_kind(
    path: &str,
    idx: usize,
    fixture: &str,
    operation_family: &str,
    hazard: &str,
    route_kind: &str,
) -> Result<(), String> {
    let cards_path = super::workspace_path("fixtures")
        .join(fixture)
        .join("expected.cards.json");
    let cards = super::parse_json_file(&cards_path)?;
    let cards = cards
        .as_array()
        .ok_or_else(|| format!("{fixture}/expected.cards.json must contain a JSON array"))?;
    let mut matched_card = false;
    for card in cards {
        if card
            .get("operation_family")
            .and_then(serde_json::Value::as_str)
            != Some(operation_family)
            || !json_array_contains_str(card, "hazards", hazard)
        {
            continue;
        }
        matched_card = true;
        for route in super::json_array_at(
            card,
            "/witness_routes",
            &format!("{fixture}/expected.cards.json"),
        )? {
            if route.get("kind").and_then(serde_json::Value::as_str) == Some(route_kind) {
                return Ok(());
            }
        }
    }
    if matched_card {
        Err(format!(
            "{path} samples[{idx}] expected_witness_route_kinds route `{route_kind}` was not found for `{operation_family}` / `{hazard}` in `{fixture}`"
        ))
    } else {
        Err(format!(
            "{path} samples[{idx}] no `{operation_family}` / `{hazard}` card was found in `{fixture}`"
        ))
    }
}

fn json_array_contains_str(value: &serde_json::Value, key: &str, expected: &str) -> bool {
    value
        .get(key)
        .and_then(serde_json::Value::as_array)
        .is_some_and(|values| values.iter().any(|value| value.as_str() == Some(expected)))
}

fn label_ledger_files() -> Result<BTreeSet<String>, String> {
    let dir = super::workspace_path(LABEL_DIR);
    if !dir.is_dir() {
        return Err(format!("{LABEL_DIR} directory is missing"));
    }
    let mut result = BTreeSet::new();
    for entry in fs::read_dir(&dir).map_err(|err| format!("failed to read {LABEL_DIR}: {err}"))? {
        let entry = entry.map_err(|err| format!("failed to enumerate {LABEL_DIR}: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("non-UTF-8 label ledger file in {LABEL_DIR}"))?;
        result.insert(format!("{LABEL_DIR}/{name}"));
    }
    Ok(result)
}

fn validate_label_ledger_path(path: &str, context: &str, idx: usize) -> Result<(), String> {
    if !path.starts_with("docs/accuracy/labels/") || !path.ends_with(".toml") {
        return Err(format!(
            "{context} claim[{idx}] label ledger path must be under docs/accuracy/labels and end with .toml: `{path}`"
        ));
    }
    let candidate = Path::new(path);
    if candidate.is_absolute() || path.contains('\\') || path.contains("..") {
        return Err(format!(
            "{context} claim[{idx}] label ledger path must be relative and use forward slashes: `{path}`"
        ));
    }
    Ok(())
}

fn required_table_string<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    idx: usize,
) -> Result<&'a str, String> {
    let Some(value) = table.get(key).and_then(toml::Value::as_str) else {
        return Err(format!("{path} entry[{idx}] is missing string `{key}`"));
    };
    if value.trim().is_empty() {
        Err(format!("{path} entry[{idx}] string `{key}` is empty"))
    } else {
        Ok(value)
    }
}

fn optional_table_string<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    idx: usize,
) -> Result<Option<&'a str>, String> {
    let Some(value) = table.get(key) else {
        return Ok(None);
    };
    let Some(value) = value.as_str() else {
        return Err(format!(
            "{path} entry[{idx}] optional `{key}` must be a string"
        ));
    };
    if value.trim().is_empty() {
        return Err(format!("{path} entry[{idx}] optional `{key}` is empty"));
    }
    Ok(Some(value))
}

fn required_table_usize(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    idx: usize,
) -> Result<usize, String> {
    let Some(value) = table.get(key).and_then(toml::Value::as_integer) else {
        return Err(format!("{path} entry[{idx}] is missing integer `{key}`"));
    };
    usize::try_from(value)
        .map_err(|err| format!("{path} entry[{idx}] integer `{key}` is invalid: {err}"))
}

fn require_exact_string(
    table: &toml::map::Map<String, toml::Value>,
    key: &str,
    expected: &str,
    path: &str,
    idx: usize,
) -> Result<(), String> {
    let actual = required_table_string(table, key, path, idx)?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{path} entry[{idx}] `{key}` is `{actual}`, expected `{expected}`"
        ))
    }
}

fn optional_table_str_array<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    idx: usize,
) -> Result<Vec<&'a str>, String> {
    let Some(value) = table.get(key) else {
        return Ok(Vec::new());
    };
    required_table_str_array_value(value, key, path, idx)
}

fn required_table_str_array<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &str,
    idx: usize,
) -> Result<Vec<&'a str>, String> {
    let Some(value) = table.get(key) else {
        return Err(format!("{path} entry[{idx}] is missing array `{key}`"));
    };
    required_table_str_array_value(value, key, path, idx)
}

fn required_table_str_array_value<'a>(
    value: &'a toml::Value,
    key: &str,
    path: &str,
    idx: usize,
) -> Result<Vec<&'a str>, String> {
    let values = value
        .as_array()
        .ok_or_else(|| format!("{path} entry[{idx}] `{key}` must be an array"))?;
    let mut result = Vec::new();
    for (array_idx, value) in values.iter().enumerate() {
        let Some(text) = value.as_str() else {
            return Err(format!(
                "{path} entry[{idx}] `{key}`[{array_idx}] must be a string"
            ));
        };
        if text.trim().is_empty() {
            return Err(format!(
                "{path} entry[{idx}] `{key}`[{array_idx}] must not be empty"
            ));
        }
        result.push(text);
    }
    Ok(result)
}

fn require_allowed(value: &str, allowed: &[&str], path: &str, key: &str) -> Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "{path} `{key}` value `{value}` is not allowed; expected one of {}",
            allowed.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_ledger_rejects_missing_witness_route_kind() -> Result<(), String> {
        let ledger = r#"
schema_version = "0.1"
status = "fixture_pinned"
claim_id = "unsafe-impl-send-sync-witness-routes"
operation_family = "unsafe_impl_send_sync"
hazard = "send_sync_invariant"
partition = "fixture"
source_kind = "fixture_golden"
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result."

[[samples]]
id = "bad-route"
fixture = "unsafe_impl_send"
kind = "positive"
expected_cards = 1
expected_class = "requires_loom"
expected_operation_family = "unsafe_impl_send_sync"
expected_hazard = "send_sync_invariant"
expected_obligation_key = "thread-safety"
expected_discharge_state = "missing"
expected_witness_route_kinds = ["asan"]
label_source = "fixture_golden"
rationale = "The fixture routes Send/Sync invariants to Loom/Shuttle, so ASan should be rejected."
"#
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("parse test ledger failed: {err}"))?;
        let mut cases = BTreeMap::new();
        cases.insert(
            "unsafe_impl_send".to_string(),
            CalibrationFixtureCase {
                kind: "positive".to_string(),
                expected_cards: 1,
                expected_class: Some("requires_loom".to_string()),
                expected_operation_family: Some("unsafe_impl_send_sync".to_string()),
                expected_hazard: Some("send_sync_invariant".to_string()),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("unsafe_impl_send_sync".to_string()),
            hazard: Some("send_sync_invariant".to_string()),
            label_ledgers: BTreeSet::new(),
        };

        let result = validate_label_ledger(
            "docs/accuracy/labels/test.toml",
            &ledger,
            "unsafe-impl-send-sync-witness-routes",
            &claim,
            &cases,
        );

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("expected_witness_route_kinds")
        );
        Ok(())
    }

    #[test]
    fn label_ledger_rejects_wrong_obligation_contract_state() -> Result<(), String> {
        let ledger = r#"
schema_version = "0.1"
status = "fixture_pinned"
claim_id = "public-unsafe-api-safety-docs-contract-evidence"
operation_family = "unknown"
hazard = "unknown"
partition = "fixture"
source_kind = "fixture_golden"
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result."

[[samples]]
id = "bad-contract-state"
fixture = "public_unsafe_fn_missing_safety"
kind = "positive"
expected_cards = 1
expected_class = "contract_missing"
expected_operation_family = "unknown"
expected_hazard = "unknown"
expected_obligation_key = "unknown"
expected_contract_state = "present"
expected_discharge_state = "present"
label_source = "fixture_golden"
rationale = "The fixture intentionally lacks public safety docs, so present contract evidence should be rejected."
"#
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("parse test ledger failed: {err}"))?;
        let mut cases = BTreeMap::new();
        cases.insert(
            "public_unsafe_fn_missing_safety".to_string(),
            CalibrationFixtureCase {
                kind: "positive".to_string(),
                expected_cards: 1,
                expected_class: Some("contract_missing".to_string()),
                expected_operation_family: Some("unknown".to_string()),
                expected_hazard: Some("unknown".to_string()),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("unknown".to_string()),
            hazard: Some("unknown".to_string()),
            label_ledgers: BTreeSet::new(),
        };

        let result = validate_label_ledger(
            "docs/accuracy/labels/test.toml",
            &ledger,
            "public-unsafe-api-safety-docs-contract-evidence",
            &claim,
            &cases,
        );

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("expected_contract_state")
        );
        Ok(())
    }

    #[test]
    fn label_ledger_rejects_wrong_obligation_discharge_state() -> Result<(), String> {
        let ledger = r#"
schema_version = "0.1"
status = "fixture_pinned"
claim_id = "raw-pointer-read-alignment-evidence"
operation_family = "raw_pointer_read"
hazard = "alignment"
partition = "fixture"
source_kind = "fixture_golden"
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result."

[[samples]]
id = "bad-state"
fixture = "raw_pointer_alignment_is_aligned_guard"
kind = "positive"
expected_cards = 1
expected_class = "guard_missing"
expected_operation_family = "raw_pointer_read"
expected_hazard = "alignment"
expected_obligation_key = "alignment"
expected_discharge_state = "missing"
label_source = "fixture_golden"
rationale = "The fixture intentionally has alignment evidence, so missing should be rejected."
"#
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("parse test ledger failed: {err}"))?;
        let mut cases = BTreeMap::new();
        cases.insert(
            "raw_pointer_alignment_is_aligned_guard".to_string(),
            CalibrationFixtureCase {
                kind: "positive".to_string(),
                expected_cards: 1,
                expected_class: Some("guard_missing".to_string()),
                expected_operation_family: Some("raw_pointer_read".to_string()),
                expected_hazard: Some("alignment".to_string()),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("raw_pointer_read".to_string()),
            hazard: Some("alignment".to_string()),
            label_ledgers: BTreeSet::new(),
        };

        let result = validate_label_ledger(
            "docs/accuracy/labels/test.toml",
            &ledger,
            "raw-pointer-read-alignment-evidence",
            &claim,
            &cases,
        );

        assert!(
            result
                .err()
                .unwrap_or_default()
                .contains("expected_discharge_state")
        );
        Ok(())
    }
}
