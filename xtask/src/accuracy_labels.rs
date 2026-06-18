use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use super::calibration_constants::OPERATION_FAMILY_REGISTRY;
use super::support_tiers::SUPPORT_TIERS_DOC;

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
    "expected_owner",
    "expected_site_kind",
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
const LABEL_TRUST_BOUNDARY_LIMITS: &[&str] = &[
    "not Miri-clean status",
    "not site execution evidence",
    "not calibrated precision or recall",
    "not witness adequacy",
    "not policy readiness",
];

#[derive(Clone, Debug)]
pub(crate) struct CalibrationFixtureCase {
    pub(crate) kind: String,
    pub(crate) expected_cards: usize,
    pub(crate) expected_class: Option<String>,
    pub(crate) expected_operation_family: Option<String>,
    pub(crate) expected_hazard: Option<String>,
    /// Surface names for which committed goldens are expected (e.g. "lsp", "repair-queue").
    /// Empty for most fixtures; non-empty only for the exemplar fixtures.
    pub(crate) surface_goldens: Vec<String>,
}

#[derive(Debug)]
struct PolicyClaim {
    operation_family: Option<String>,
    hazard: Option<String>,
    fixtures: BTreeSet<String>,
    label_ledgers: BTreeSet<String>,
}

struct LabelLedgerStats {
    sample_count: usize,
    fixtures: BTreeSet<String>,
    sample_keys: BTreeSet<String>,
}

struct LabelSampleStats {
    fixture: String,
    key: String,
}

struct FixtureObligation<'a> {
    fixture: &'a str,
    operation_family: &'a str,
    hazard: &'a str,
    expected_owner: Option<&'a str>,
    expected_site_kind: Option<&'a str>,
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
    let mut label_fixture_refs = BTreeMap::new();
    for (claim_id, claim) in &claims {
        let mut claim_sample_fixtures = BTreeSet::new();
        let mut claim_sample_keys = BTreeSet::new();
        for ledger in &claim.label_ledgers {
            if !actual_ledgers.contains(ledger) {
                return Err(format!(
                    "policy/accuracy-calibration.toml claim `{claim_id}` references missing label ledger `{ledger}`"
                ));
            }
            let value = super::parse_toml_file(&super::workspace_path(ledger))?;
            let stats = validate_label_ledger(ledger, &value, claim_id, claim, fixture_cases)?;
            sample_count += stats.sample_count;
            label_fixture_refs.insert(ledger.clone(), stats.fixtures.clone());
            extend_claim_label_samples(
                claim_id,
                ledger,
                &mut claim_sample_fixtures,
                &mut claim_sample_keys,
                stats,
            )?;
        }
        check_policy_claim_fixture_sample_coverage(claim_id, claim, &claim_sample_fixtures)?;
    }
    let support_tiers = super::read_to_string(&super::workspace_path(SUPPORT_TIERS_DOC))?;
    let operation_registry =
        super::read_to_string(&super::workspace_path(OPERATION_FAMILY_REGISTRY))?;
    check_label_fixture_doc_coverage(&label_fixture_refs, &support_tiers, &operation_registry)?;

    Ok(sample_count)
}

fn check_label_fixture_doc_coverage(
    label_fixture_refs: &BTreeMap<String, BTreeSet<String>>,
    support_tiers: &str,
    operation_registry: &str,
) -> Result<(), String> {
    for (ledger, fixtures) in label_fixture_refs {
        for fixture in fixtures {
            if !support_tiers.contains(fixture) {
                return Err(format!(
                    "{ledger} fixture `{fixture}` is not referenced by {SUPPORT_TIERS_DOC}"
                ));
            }
            if !operation_registry.contains(fixture) {
                return Err(format!(
                    "{ledger} fixture `{fixture}` is not referenced by {OPERATION_FAMILY_REGISTRY}"
                ));
            }
        }
    }
    Ok(())
}

fn extend_claim_label_samples(
    claim_id: &str,
    ledger: &str,
    claim_sample_fixtures: &mut BTreeSet<String>,
    claim_sample_keys: &mut BTreeSet<String>,
    ledger_stats: LabelLedgerStats,
) -> Result<(), String> {
    claim_sample_fixtures.extend(ledger_stats.fixtures);
    for key in ledger_stats.sample_keys {
        if !claim_sample_keys.insert(key.clone()) {
            return Err(format!(
                "policy/accuracy-calibration.toml claim `{claim_id}` has duplicate label sample `{key}` across label ledgers including `{ledger}`"
            ));
        }
    }
    Ok(())
}

fn check_policy_claim_fixture_sample_coverage(
    claim_id: &str,
    claim: &PolicyClaim,
    sample_fixtures: &BTreeSet<String>,
) -> Result<(), String> {
    let missing_sample_fixtures = claim
        .fixtures
        .difference(sample_fixtures)
        .cloned()
        .collect::<Vec<_>>();
    if !missing_sample_fixtures.is_empty() {
        return Err(format!(
            "policy/accuracy-calibration.toml claim `{claim_id}` lists fixture(s) without label samples: {}",
            missing_sample_fixtures.join(", ")
        ));
    }
    Ok(())
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
        let fixtures =
            optional_table_str_array(claim, "fixtures", "policy/accuracy-calibration.toml", idx)?;
        let mut claimed_fixtures = BTreeSet::new();
        for fixture in fixtures {
            if !claimed_fixtures.insert(fixture.to_string()) {
                return Err(format!(
                    "policy/accuracy-calibration.toml claim[{idx}] duplicates fixture `{fixture}`"
                ));
            }
        }
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
                    fixtures: claimed_fixtures,
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
) -> Result<LabelLedgerStats, String> {
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
    require_label_trust_boundary_limits(boundary, path)?;
    let samples = super::toml_array(value, "samples", path)?;
    if samples.is_empty() {
        return Err(format!("{path} must contain at least one sample"));
    }
    let mut ids = BTreeSet::new();
    let mut fixtures = BTreeSet::new();
    let mut sample_keys = BTreeSet::new();
    let context = LabelLedgerContext {
        path,
        claim_id,
        source_kind,
        claim,
        fixture_cases,
    };
    for (idx, sample) in samples.iter().enumerate() {
        let sample = sample
            .as_table()
            .ok_or_else(|| format!("{path} samples[{idx}] must be a TOML table"))?;
        let sample = validate_sample(&context, idx, sample, &mut ids)?;
        fixtures.insert(sample.fixture);
        if !sample_keys.insert(sample.key.clone()) {
            return Err(format!("{path} contains duplicate sample `{}`", sample.key));
        }
    }
    Ok(LabelLedgerStats {
        sample_count: samples.len(),
        fixtures,
        sample_keys,
    })
}

fn require_label_trust_boundary_limits(text: &str, path: &str) -> Result<(), String> {
    for needle in LABEL_TRUST_BOUNDARY_LIMITS {
        if !super::text_contains_ignore_ascii_case(text, needle) {
            return Err(format!("{path} trust boundary is missing `{needle}`"));
        }
    }
    Ok(())
}

struct LabelLedgerContext<'a> {
    path: &'a str,
    claim_id: &'a str,
    source_kind: &'a str,
    claim: &'a PolicyClaim,
    fixture_cases: &'a BTreeMap<String, CalibrationFixtureCase>,
}

fn validate_sample(
    context: &LabelLedgerContext<'_>,
    idx: usize,
    sample: &toml::map::Map<String, toml::Value>,
    ids: &mut BTreeSet<String>,
) -> Result<LabelSampleStats, String> {
    let path = context.path;
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
    if label_source != context.source_kind {
        return Err(format!(
            "{path} samples[{idx}] label_source `{label_source}` does not match ledger source_kind `{}`",
            context.source_kind
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
    if !context.claim.fixtures.contains(fixture) {
        return Err(format!(
            "{path} samples[{idx}] references fixture `{fixture}` not listed by policy/accuracy-calibration.toml claim"
        ));
    }
    let fixture_case = context.fixture_cases.get(fixture).ok_or_else(|| {
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
        return Ok(LabelSampleStats {
            fixture: fixture.to_string(),
            key: format!("{fixture}|zero-card"),
        });
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
    let expected_owner = optional_table_string(sample, "expected_owner", path, idx)?;
    let expected_site_kind = optional_table_string(sample, "expected_site_kind", path, idx)?;
    let obligation_key = required_table_string(sample, "expected_obligation_key", path, idx)?;
    require_obligation_hazard_alignment(path, idx, obligation_key, expected_hazard)?;
    let fixture_obligation = FixtureObligation {
        fixture,
        operation_family: expected_operation_family,
        hazard: expected_hazard,
        expected_owner,
        expected_site_kind,
        obligation_key,
    };
    let contract_state = optional_table_string(sample, "expected_contract_state", path, idx)?;
    if public_contract_claim_requires_contract_state(context.claim_id) && contract_state.is_none() {
        return Err(format!(
            "{path} samples[{idx}] claim `{}` must pin `expected_contract_state`",
            context.claim_id
        ));
    }
    if let Some(contract_state) = contract_state {
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
    if witness_route_claim_requires_route_labels(context.claim_id) && route_kinds.is_empty() {
        return Err(format!(
            "{path} samples[{idx}] claim `{}` must pin `expected_witness_route_kinds`",
            context.claim_id
        ));
    }
    if !route_kinds.is_empty() {
        for route_kind in &route_kinds {
            require_allowed(
                route_kind,
                WITNESS_ROUTE_KINDS,
                path,
                "expected_witness_route_kinds",
            )?;
            check_fixture_witness_route_kind(path, idx, &fixture_obligation, route_kind)?;
        }
    }
    Ok(LabelSampleStats {
        fixture: fixture.to_string(),
        key: format!(
            "{}|{}|{}|{}|contract={}|discharge={}|routes={}",
            fixture,
            expected_operation_family,
            expected_hazard,
            obligation_key,
            contract_state.unwrap_or(""),
            discharge_state,
            route_kinds.join(",")
        ),
    })
}

fn public_contract_claim_requires_contract_state(claim_id: &str) -> bool {
    claim_id == "public-unsafe-api-safety-docs-contract-evidence"
}

fn witness_route_claim_requires_route_labels(claim_id: &str) -> bool {
    claim_id.ends_with("-witness-routes") || claim_id.ends_with("-human-review-routes")
}

fn require_obligation_hazard_alignment(
    path: &str,
    idx: usize,
    obligation_key: &str,
    expected_hazard: &str,
) -> Result<(), String> {
    if obligation_key == "valid-range" && expected_hazard != "bounds" {
        return Err(format!(
            "{path} samples[{idx}] expected_obligation_key `valid-range` must use expected_hazard `bounds`, got `{expected_hazard}`"
        ));
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
        "expected_owner",
        "expected_site_kind",
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
        if !card_matches_obligation_selector(card, obligation) {
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
            "{path} samples[{idx}] expected_obligation_key `{}` was not found for {} in `{}`",
            obligation.obligation_key,
            obligation_selector_description(obligation),
            obligation.fixture
        ))
    }
}

fn check_fixture_witness_route_kind(
    path: &str,
    idx: usize,
    obligation: &FixtureObligation<'_>,
    route_kind: &str,
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
    let mut matched_card = false;
    for card in cards {
        if !card_matches_obligation_selector(card, obligation) {
            continue;
        }
        matched_card = true;
        for route in super::json_array_at(
            card,
            "/witness_routes",
            &format!("{}/expected.cards.json", obligation.fixture),
        )? {
            if route.get("kind").and_then(serde_json::Value::as_str) == Some(route_kind) {
                return Ok(());
            }
        }
    }
    if matched_card {
        Err(format!(
            "{path} samples[{idx}] expected_witness_route_kinds route `{route_kind}` was not found for {} in `{}`",
            obligation_selector_description(obligation),
            obligation.fixture
        ))
    } else {
        Err(format!(
            "{path} samples[{idx}] no card matching {} was found in `{}`",
            obligation_selector_description(obligation),
            obligation.fixture
        ))
    }
}

fn card_matches_obligation_selector(
    card: &serde_json::Value,
    obligation: &FixtureObligation<'_>,
) -> bool {
    if card
        .get("operation_family")
        .and_then(serde_json::Value::as_str)
        != Some(obligation.operation_family)
        || !json_array_contains_str(card, "hazards", obligation.hazard)
    {
        return false;
    }
    if let Some(expected_owner) = obligation.expected_owner
        && card
            .pointer("/site/owner")
            .and_then(serde_json::Value::as_str)
            != Some(expected_owner)
    {
        return false;
    }
    if let Some(expected_site_kind) = obligation.expected_site_kind
        && card
            .pointer("/site/kind")
            .and_then(serde_json::Value::as_str)
            != Some(expected_site_kind)
    {
        return false;
    }
    true
}

fn obligation_selector_description(obligation: &FixtureObligation<'_>) -> String {
    let mut parts = vec![
        format!("operation `{}`", obligation.operation_family),
        format!("hazard `{}`", obligation.hazard),
    ];
    if let Some(expected_owner) = obligation.expected_owner {
        parts.push(format!("expected_owner `{expected_owner}`"));
    }
    if let Some(expected_site_kind) = obligation.expected_site_kind {
        parts.push(format!("expected_site_kind `{expected_site_kind}`"));
    }
    parts.join(" / ")
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
    fn label_ledger_rejects_incomplete_trust_boundary_limits() -> Result<(), String> {
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
id = "minimal-boundary"
fixture = "raw_pointer_alignment"
kind = "positive"
expected_cards = 1
expected_class = "guard_missing"
expected_operation_family = "raw_pointer_read"
expected_hazard = "alignment"
expected_obligation_key = "alignment"
expected_discharge_state = "missing"
label_source = "fixture_golden"
rationale = "The trust boundary must name the full accuracy-label no-overclaim posture."
"#
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("parse test ledger failed: {err}"))?;
        let mut cases = BTreeMap::new();
        cases.insert(
            "raw_pointer_alignment".to_string(),
            CalibrationFixtureCase {
                kind: "positive".to_string(),
                expected_cards: 1,
                expected_class: Some("guard_missing".to_string()),
                expected_operation_family: Some("raw_pointer_read".to_string()),
                expected_hazard: Some("alignment".to_string()),
                surface_goldens: Vec::new(),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("raw_pointer_read".to_string()),
            hazard: Some("alignment".to_string()),
            fixtures: BTreeSet::from(["raw_pointer_alignment".to_string()]),
            label_ledgers: BTreeSet::new(),
        };

        let result = validate_label_ledger(
            "docs/accuracy/labels/test.toml",
            &ledger,
            "raw-pointer-read-alignment-evidence",
            &claim,
            &cases,
        );

        let err = result.err().unwrap_or_default();
        assert!(err.contains("trust boundary"));
        assert!(err.contains("not Miri-clean status"));
        Ok(())
    }

    #[test]
    fn label_ledger_rejects_witness_route_claim_without_route_labels() -> Result<(), String> {
        let ledger = r#"
schema_version = "0.1"
status = "fixture_pinned"
claim_id = "unsafe-impl-send-sync-witness-routes"
operation_family = "unsafe_impl_send_sync"
hazard = "send_sync_invariant"
partition = "fixture"
source_kind = "fixture_golden"
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site execution evidence, not calibrated precision or recall, not witness adequacy, and not policy readiness."

[[samples]]
id = "missing-route-labels"
fixture = "unsafe_impl_send"
kind = "positive"
expected_cards = 1
expected_class = "requires_loom"
expected_operation_family = "unsafe_impl_send_sync"
expected_hazard = "send_sync_invariant"
expected_obligation_key = "thread-safety"
expected_discharge_state = "missing"
label_source = "fixture_golden"
rationale = "Witness-route calibration claims must pin the route kinds projected by the ReviewCard."
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
                surface_goldens: Vec::new(),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("unsafe_impl_send_sync".to_string()),
            hazard: Some("send_sync_invariant".to_string()),
            fixtures: BTreeSet::from(["unsafe_impl_send".to_string()]),
            label_ledgers: BTreeSet::new(),
        };

        let result = validate_label_ledger(
            "docs/accuracy/labels/test.toml",
            &ledger,
            "unsafe-impl-send-sync-witness-routes",
            &claim,
            &cases,
        );

        let err = result.err().unwrap_or_default();
        assert!(err.contains("must pin"));
        assert!(err.contains("expected_witness_route_kinds"));
        Ok(())
    }

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
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site execution evidence, not calibrated precision or recall, not witness adequacy, and not policy readiness."

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
                surface_goldens: Vec::new(),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("unsafe_impl_send_sync".to_string()),
            hazard: Some("send_sync_invariant".to_string()),
            fixtures: BTreeSet::from(["unsafe_impl_send".to_string()]),
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
    fn label_ledger_rejects_public_contract_claim_without_contract_state() -> Result<(), String> {
        let ledger = r#"
schema_version = "0.1"
status = "fixture_pinned"
claim_id = "public-unsafe-api-safety-docs-contract-evidence"
operation_family = "unsafe_declaration"
hazard = "unknown"
partition = "fixture"
source_kind = "fixture_golden"
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site execution evidence, not calibrated precision or recall, not witness adequacy, and not policy readiness."

[[samples]]
id = "missing-contract-state"
fixture = "public_unsafe_fn_missing_safety"
kind = "positive"
expected_cards = 1
expected_class = "contract_missing"
expected_operation_family = "unsafe_declaration"
expected_hazard = "unknown"
expected_obligation_key = "caller-contract"
expected_discharge_state = "present"
label_source = "fixture_golden"
rationale = "Public unsafe API contract evidence claims must pin the ReviewCard contract evidence state."
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
                expected_operation_family: Some("unsafe_declaration".to_string()),
                expected_hazard: Some("unknown".to_string()),
                surface_goldens: Vec::new(),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("unsafe_declaration".to_string()),
            hazard: Some("unknown".to_string()),
            fixtures: BTreeSet::from(["public_unsafe_fn_missing_safety".to_string()]),
            label_ledgers: BTreeSet::new(),
        };

        let result = validate_label_ledger(
            "docs/accuracy/labels/test.toml",
            &ledger,
            "public-unsafe-api-safety-docs-contract-evidence",
            &claim,
            &cases,
        );

        let err = result.err().unwrap_or_default();
        assert!(err.contains("must pin"));
        assert!(err.contains("expected_contract_state"));
        Ok(())
    }

    #[test]
    fn label_ledger_rejects_wrong_obligation_contract_state() -> Result<(), String> {
        // Uses public_unsafe_trait_missing_safety: still emits a declaration card in diff scope.
        // The golden has contract.state = "missing";
        // asserting "present" should be rejected by check_fixture_obligation_evidence_state.
        let ledger = r#"
schema_version = "0.1"
status = "fixture_pinned"
claim_id = "public-unsafe-api-safety-docs-contract-evidence"
operation_family = "unsafe_declaration"
hazard = "unknown"
partition = "fixture"
source_kind = "fixture_golden"
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site execution evidence, not calibrated precision or recall, not witness adequacy, and not policy readiness."

[[samples]]
id = "bad-contract-state"
fixture = "public_unsafe_trait_missing_safety"
kind = "positive"
expected_cards = 1
expected_class = "contract_missing"
expected_operation_family = "unsafe_declaration"
expected_hazard = "unknown"
expected_obligation_key = "caller-contract"
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
            "public_unsafe_trait_missing_safety".to_string(),
            CalibrationFixtureCase {
                kind: "positive".to_string(),
                expected_cards: 1,
                expected_class: Some("contract_missing".to_string()),
                expected_operation_family: Some("unsafe_declaration".to_string()),
                expected_hazard: Some("unknown".to_string()),
                surface_goldens: Vec::new(),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("unsafe_declaration".to_string()),
            hazard: Some("unknown".to_string()),
            fixtures: BTreeSet::from(["public_unsafe_trait_missing_safety".to_string()]),
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
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site execution evidence, not calibrated precision or recall, not witness adequacy, and not policy readiness."

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
                surface_goldens: Vec::new(),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("raw_pointer_read".to_string()),
            hazard: Some("alignment".to_string()),
            fixtures: BTreeSet::from(["raw_pointer_alignment_is_aligned_guard".to_string()]),
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

    #[test]
    fn label_ledger_rejects_valid_range_sample_with_non_bounds_hazard() -> Result<(), String> {
        let ledger = r#"
schema_version = "0.1"
status = "fixture_pinned"
claim_id = "ptr-copy-valid-range-evidence"
operation_family = "ptr_copy"
hazard = "bounds"
partition = "fixture"
source_kind = "fixture_golden"
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site execution evidence, not calibrated precision or recall, not witness adequacy, and not policy readiness."

[[samples]]
id = "bad-valid-range-hazard"
fixture = "ptr_copy_slice_range_guard"
kind = "positive"
expected_cards = 1
expected_class = "guard_missing"
expected_operation_family = "ptr_copy"
expected_hazard = "pointer_validity"
expected_obligation_key = "valid-range"
expected_discharge_state = "present"
label_source = "fixture_golden"
rationale = "Valid-range labels must stay tied to the bounds hazard, not a broader pointer validity selector."
"#
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("parse test ledger failed: {err}"))?;
        let mut cases = BTreeMap::new();
        cases.insert(
            "ptr_copy_slice_range_guard".to_string(),
            CalibrationFixtureCase {
                kind: "positive".to_string(),
                expected_cards: 1,
                expected_class: Some("guard_missing".to_string()),
                expected_operation_family: Some("ptr_copy".to_string()),
                expected_hazard: Some("pointer_validity".to_string()),
                surface_goldens: Vec::new(),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("ptr_copy".to_string()),
            hazard: Some("bounds".to_string()),
            fixtures: BTreeSet::from(["ptr_copy_slice_range_guard".to_string()]),
            label_ledgers: BTreeSet::new(),
        };

        let result = validate_label_ledger(
            "docs/accuracy/labels/test.toml",
            &ledger,
            "ptr-copy-valid-range-evidence",
            &claim,
            &cases,
        );

        let err = result.err().unwrap_or_default();
        assert!(err.contains("expected_obligation_key `valid-range`"));
        assert!(err.contains("expected_hazard `bounds`"));
        Ok(())
    }

    #[test]
    fn label_ledger_rejects_wrong_card_owner() -> Result<(), String> {
        let ledger = r#"
schema_version = "0.1"
status = "fixture_pinned"
claim_id = "raw-pointer-read-alignment-evidence"
operation_family = "raw_pointer_read"
hazard = "alignment"
partition = "fixture"
source_kind = "fixture_golden"
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site execution evidence, not calibrated precision or recall, not witness adequacy, and not policy readiness."

[[samples]]
id = "bad-owner"
fixture = "raw_pointer_alignment_is_aligned_guard"
kind = "positive"
expected_cards = 1
expected_class = "guard_missing"
expected_operation_family = "raw_pointer_read"
expected_hazard = "alignment"
expected_owner = "wrong_owner"
expected_obligation_key = "alignment"
expected_discharge_state = "present"
label_source = "fixture_golden"
rationale = "The fixture owner should be checked as part of ReviewCard identity calibration."
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
                surface_goldens: Vec::new(),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("raw_pointer_read".to_string()),
            hazard: Some("alignment".to_string()),
            fixtures: BTreeSet::from(["raw_pointer_alignment_is_aligned_guard".to_string()]),
            label_ledgers: BTreeSet::new(),
        };

        let result = validate_label_ledger(
            "docs/accuracy/labels/test.toml",
            &ledger,
            "raw-pointer-read-alignment-evidence",
            &claim,
            &cases,
        );

        assert!(result.err().unwrap_or_default().contains("expected_owner"));
        Ok(())
    }

    #[test]
    fn label_ledger_rejects_sample_outside_policy_claim_fixture_list() -> Result<(), String> {
        let ledger = r#"
schema_version = "0.1"
status = "fixture_pinned"
claim_id = "raw-pointer-read-alignment-evidence"
operation_family = "raw_pointer_read"
hazard = "alignment"
partition = "fixture"
source_kind = "fixture_golden"
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site execution evidence, not calibrated precision or recall, not witness adequacy, and not policy readiness."

[[samples]]
id = "outside-claim-fixture"
fixture = "raw_pointer_alignment_is_aligned_guard"
kind = "positive"
expected_cards = 1
expected_class = "guard_missing"
expected_operation_family = "raw_pointer_read"
expected_hazard = "alignment"
expected_obligation_key = "alignment"
expected_discharge_state = "present"
label_source = "fixture_golden"
rationale = "The fixture is valid calibration data, but this claim did not list it."
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
                surface_goldens: Vec::new(),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("raw_pointer_read".to_string()),
            hazard: Some("alignment".to_string()),
            fixtures: BTreeSet::from(["raw_pointer_alignment".to_string()]),
            label_ledgers: BTreeSet::new(),
        };

        let result = validate_label_ledger(
            "docs/accuracy/labels/test.toml",
            &ledger,
            "raw-pointer-read-alignment-evidence",
            &claim,
            &cases,
        );

        let err = result.err().unwrap_or_default();
        assert!(err.contains("not listed"));
        assert!(err.contains("raw_pointer_alignment_is_aligned_guard"));
        Ok(())
    }

    #[test]
    fn policy_claim_fixture_coverage_rejects_unsampled_claim_fixture() -> Result<(), String> {
        let claim = PolicyClaim {
            operation_family: Some("raw_pointer_read".to_string()),
            hazard: Some("alignment".to_string()),
            fixtures: BTreeSet::from([
                "raw_pointer_alignment".to_string(),
                "raw_pointer_alignment_is_aligned_guard".to_string(),
            ]),
            label_ledgers: BTreeSet::new(),
        };
        let sample_fixtures = BTreeSet::from(["raw_pointer_alignment".to_string()]);

        let Err(err) = check_policy_claim_fixture_sample_coverage(
            "raw-pointer-read-alignment-evidence",
            &claim,
            &sample_fixtures,
        ) else {
            return Err("claim fixture without label sample should fail".to_string());
        };

        assert!(err.contains("without label samples"));
        assert!(err.contains("raw_pointer_alignment_is_aligned_guard"));
        Ok(())
    }

    #[test]
    fn label_fixture_doc_coverage_accepts_support_and_registry_mentions() -> Result<(), String> {
        let refs = BTreeMap::from([(
            "docs/accuracy/labels/raw-pointer-read-alignment.toml".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);

        check_label_fixture_doc_coverage(
            &refs,
            "support row references raw_pointer_alignment",
            "registry fixture proof references raw_pointer_alignment",
        )
    }

    #[test]
    fn label_fixture_doc_coverage_rejects_missing_support_tier_reference() -> Result<(), String> {
        let refs = BTreeMap::from([(
            "docs/accuracy/labels/raw-pointer-read-alignment.toml".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);

        let Err(err) = check_label_fixture_doc_coverage(
            &refs,
            "support row omits the fixture",
            "registry fixture proof references raw_pointer_alignment",
        ) else {
            return Err("missing support-tier fixture reference should fail".to_string());
        };

        assert!(err.contains("docs/accuracy/labels/raw-pointer-read-alignment.toml"));
        assert!(err.contains("raw_pointer_alignment"));
        assert!(err.contains(SUPPORT_TIERS_DOC));
        Ok(())
    }

    #[test]
    fn label_fixture_doc_coverage_rejects_missing_registry_reference() -> Result<(), String> {
        let refs = BTreeMap::from([(
            "docs/accuracy/labels/raw-pointer-read-alignment.toml".to_string(),
            BTreeSet::from(["raw_pointer_alignment".to_string()]),
        )]);

        let Err(err) = check_label_fixture_doc_coverage(
            &refs,
            "support row references raw_pointer_alignment",
            "registry fixture proof omits the fixture",
        ) else {
            return Err(
                "missing operation-family registry fixture reference should fail".to_string(),
            );
        };

        assert!(err.contains("docs/accuracy/labels/raw-pointer-read-alignment.toml"));
        assert!(err.contains("raw_pointer_alignment"));
        assert!(err.contains(OPERATION_FAMILY_REGISTRY));
        Ok(())
    }

    #[test]
    fn label_ledger_rejects_duplicate_sample_fixture() -> Result<(), String> {
        let ledger = r#"
schema_version = "0.1"
status = "fixture_pinned"
claim_id = "raw-pointer-read-alignment-evidence"
operation_family = "raw_pointer_read"
hazard = "alignment"
partition = "fixture"
source_kind = "fixture_golden"
trust_boundary = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site execution evidence, not calibrated precision or recall, not witness adequacy, and not policy readiness."

[[samples]]
id = "first"
fixture = "raw_pointer_alignment_is_aligned_guard"
kind = "positive"
expected_cards = 1
expected_class = "guard_missing"
expected_operation_family = "raw_pointer_read"
expected_hazard = "alignment"
expected_obligation_key = "alignment"
expected_discharge_state = "present"
label_source = "fixture_golden"
rationale = "The fixture has same-pointer alignment evidence before the raw pointer read."

[[samples]]
id = "second"
fixture = "raw_pointer_alignment_is_aligned_guard"
kind = "positive"
expected_cards = 1
expected_class = "guard_missing"
expected_operation_family = "raw_pointer_read"
expected_hazard = "alignment"
expected_obligation_key = "alignment"
expected_discharge_state = "present"
label_source = "fixture_golden"
rationale = "A second sample for the same fixture would overstate the claim sample count."
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
                surface_goldens: Vec::new(),
            },
        );
        let claim = PolicyClaim {
            operation_family: Some("raw_pointer_read".to_string()),
            hazard: Some("alignment".to_string()),
            fixtures: BTreeSet::from(["raw_pointer_alignment_is_aligned_guard".to_string()]),
            label_ledgers: BTreeSet::new(),
        };

        let result = validate_label_ledger(
            "docs/accuracy/labels/test.toml",
            &ledger,
            "raw-pointer-read-alignment-evidence",
            &claim,
            &cases,
        );

        let err = result.err().unwrap_or_default();
        assert!(err.contains("duplicate sample"));
        assert!(err.contains("raw_pointer_alignment_is_aligned_guard"));
        Ok(())
    }

    #[test]
    fn claim_fixture_coverage_rejects_duplicate_sample_across_ledgers() -> Result<(), String> {
        let key = "raw_pointer_alignment|raw_pointer_read|alignment|alignment|contract=|discharge=missing|routes=";
        let mut seen_fixtures = BTreeSet::from(["raw_pointer_alignment".to_string()]);
        let mut seen_keys = BTreeSet::from([key.to_string()]);
        let ledger_stats = LabelLedgerStats {
            sample_count: 1,
            fixtures: BTreeSet::from(["raw_pointer_alignment".to_string()]),
            sample_keys: BTreeSet::from([key.to_string()]),
        };

        let Err(err) = extend_claim_label_samples(
            "raw-pointer-read-alignment-evidence",
            "docs/accuracy/labels/duplicate.toml",
            &mut seen_fixtures,
            &mut seen_keys,
            ledger_stats,
        ) else {
            return Err("duplicate fixture sample across ledgers should fail".to_string());
        };

        assert!(err.contains("duplicate label sample"));
        assert!(err.contains("raw_pointer_alignment"));
        Ok(())
    }
}
