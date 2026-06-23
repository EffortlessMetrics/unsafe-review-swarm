use super::{
    CALIBRATION_REQUIRED_KINDS, FIXTURE_EXPECTED_CARDS_EXCEPTIONS, SUPPORT_TIERS_DOC,
    accuracy_labels, check_calibration_case, check_calibration_case_fields,
    check_operation_family_registry_coverage, fixture_dir_name, fixture_dirs, optional_case_string,
    parse_toml_file, require_toml_string, required_case_string, required_case_usize,
    support_tier_capabilities, workspace_path,
};
use std::collections::{BTreeMap, BTreeSet};

const CALIBRATION_MANIFEST: &str = "fixtures/calibration.toml";

pub(crate) struct CalibrationManifest {
    pub(crate) case_count: usize,
    pub(crate) fixture_cases: BTreeMap<String, accuracy_labels::CalibrationFixtureCase>,
}

struct CalibrationCaseIndex {
    fixtures: BTreeSet<String>,
    kinds: BTreeSet<String>,
    operation_families: BTreeSet<String>,
    operation_family_fixtures: BTreeMap<String, BTreeSet<String>>,
    fixture_cases: BTreeMap<String, accuracy_labels::CalibrationFixtureCase>,
}

pub(crate) fn validate() -> Result<CalibrationManifest, String> {
    let value = parse_toml_file(&workspace_path(CALIBRATION_MANIFEST))?;
    require_toml_string(&value, "schema_version", CALIBRATION_MANIFEST)?;
    let required = required_core_fixtures(&value)?;
    let monolith_cases = calibration_cases(&value)?;

    // #1712 per-fixture registration: scan for sidecar `calibration.toml` files
    // inside each fixture directory. Sidecars let a fixture PR register its case
    // in `fixtures/<name>/calibration.toml` instead of the monolithic
    // `fixtures/calibration.toml`, so parallel fixture PRs that touch different
    // fixtures do not conflict. Sidecar cases are merged with the monolith cases;
    // a fixture registered in BOTH is a blocking double-registration error.
    // Sidecar fixtures are also auto-added to required_core_fixtures so they
    // pass the fixture-parity check without editing the monolith's array.
    let sidecar_cases = collect_sidecar_cases()?;
    let mut all_case_fixtures: BTreeSet<String> = BTreeSet::new();
    for case in monolith_cases {
        if let Some(fixture) = case.get("fixture").and_then(toml::Value::as_str) {
            all_case_fixtures.insert(fixture.to_string());
        }
    }
    let mut merged_cases: Vec<toml::Value> = monolith_cases.to_vec();
    let mut sidecar_required: BTreeSet<String> = BTreeSet::new();
    for (sidecar_path, sidecar_case) in &sidecar_cases {
        let fixture = sidecar_case
            .get("fixture")
            .and_then(toml::Value::as_str)
            .unwrap_or("");
        if all_case_fixtures.contains(fixture) {
            return Err(format!(
                "{CALIBRATION_MANIFEST}: fixture `{fixture}` is registered in both the monolith and a sidecar ({sidecar_path}); remove one registration"
            ));
        }
        all_case_fixtures.insert(fixture.to_string());
        sidecar_required.insert(fixture.to_string());
        merged_cases.push(sidecar_case.clone());
    }

    let index = index_calibration_cases(&merged_cases)?;
    require_all_calibration_kinds(&index.kinds)?;
    require_required_core_fixture_parity_with_sidecars(
        required,
        &index.fixtures,
        &sidecar_required,
    )?;
    require_expected_card_fixture_coverage(&index.fixtures)?;

    Ok(CalibrationManifest {
        case_count: merged_cases.len(),
        fixture_cases: index.fixture_cases,
    })
}

/// Scan fixture directories for sidecar `calibration.toml` files (#1712).
/// Each sidecar may declare one `[[cases]]` entry for its fixture. Returns
/// `(sidecar_path, parsed_case_table)` pairs for merging into the monolith.
fn collect_sidecar_cases() -> Result<Vec<(String, toml::Value)>, String> {
    let mut sidecars = Vec::new();
    for dir in fixture_dirs(&workspace_path("fixtures"))? {
        let sidecar = dir.join("calibration.toml");
        if !sidecar.is_file() {
            continue;
        }
        let rel = sidecar
            .strip_prefix(workspace_path(""))
            .unwrap_or(&sidecar)
            .display()
            .to_string();
        let value = parse_toml_file(&sidecar)?;
        let cases = value
            .get("cases")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{rel}: sidecar must contain a `[[cases]]` entry"))?;
        if cases.len() != 1 {
            return Err(format!(
                "{rel}: sidecar must contain exactly one `[[cases]]` entry (found {})",
                cases.len()
            ));
        }
        sidecars.push((rel, cases[0].clone()));
    }
    Ok(sidecars)
}

fn required_core_fixtures(value: &toml::Value) -> Result<&Vec<toml::Value>, String> {
    value
        .get("required_core_fixtures")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{CALIBRATION_MANIFEST} is missing required_core_fixtures"))
}

fn calibration_cases(value: &toml::Value) -> Result<&Vec<toml::Value>, String> {
    let cases = value
        .get("cases")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{CALIBRATION_MANIFEST} is missing cases"))?;
    if cases.is_empty() {
        return Err(format!("{CALIBRATION_MANIFEST} has no calibration cases"));
    }
    Ok(cases)
}

fn index_calibration_cases(cases: &[toml::Value]) -> Result<CalibrationCaseIndex, String> {
    let support_capabilities = support_tier_capabilities()?;
    let mut index = CalibrationCaseIndex {
        fixtures: BTreeSet::new(),
        kinds: BTreeSet::new(),
        operation_families: BTreeSet::new(),
        operation_family_fixtures: BTreeMap::new(),
        fixture_cases: BTreeMap::new(),
    };

    for (idx, case) in cases.iter().enumerate() {
        let Some(case) = case.as_table() else {
            return Err(format!(
                "{CALIBRATION_MANIFEST} cases[{idx}] must be a TOML table"
            ));
        };
        index_one_case(case, idx, &support_capabilities, &mut index)?;
    }

    check_operation_family_registry_coverage(
        &index.operation_families,
        &index.operation_family_fixtures,
    )?;
    Ok(index)
}

fn index_one_case(
    case: &toml::map::Map<String, toml::Value>,
    idx: usize,
    support_capabilities: &BTreeSet<String>,
    index: &mut CalibrationCaseIndex,
) -> Result<(), String> {
    check_calibration_case_fields(case, idx)?;
    let fixture = required_case_string(case, "fixture", idx)?;
    let kind = required_case_string(case, "kind", idx)?;
    let claim = required_case_string(case, "claim", idx)?;
    let support_tier = required_case_string(case, "support_tier", idx)?;
    require_known_support_tier(support_tier, idx, support_capabilities)?;
    require_known_case_kind(kind, idx)?;
    require_bounded_claim(claim, idx)?;
    require_unique_fixture(&mut index.fixtures, fixture)?;

    index.kinds.insert(kind.to_string());
    let fixture_case = fixture_case_from_table(case, kind, idx)?;
    check_calibration_case(case, fixture, kind, idx)?;
    index
        .fixture_cases
        .insert(fixture.to_string(), fixture_case);
    index_operation_family(case, idx, fixture, index)?;
    Ok(())
}

fn require_known_support_tier(
    support_tier: &str,
    idx: usize,
    support_capabilities: &BTreeSet<String>,
) -> Result<(), String> {
    if support_capabilities.contains(support_tier) {
        Ok(())
    } else {
        Err(format!(
            "{CALIBRATION_MANIFEST} cases[{idx}] support_tier `{support_tier}` is not a capability in {SUPPORT_TIERS_DOC}"
        ))
    }
}

fn require_known_case_kind(kind: &str, idx: usize) -> Result<(), String> {
    if CALIBRATION_REQUIRED_KINDS.contains(&kind) {
        Ok(())
    } else {
        Err(format!(
            "{CALIBRATION_MANIFEST} cases[{idx}] uses unknown kind `{kind}`"
        ))
    }
}

fn require_bounded_claim(claim: &str, idx: usize) -> Result<(), String> {
    if claim.len() >= 16 {
        Ok(())
    } else {
        Err(format!(
            "{CALIBRATION_MANIFEST} cases[{idx}] claim is too terse"
        ))
    }
}

fn require_unique_fixture(fixtures: &mut BTreeSet<String>, fixture: &str) -> Result<(), String> {
    if fixtures.insert(fixture.to_string()) {
        Ok(())
    } else {
        Err(format!(
            "{CALIBRATION_MANIFEST} contains duplicate fixture `{fixture}`"
        ))
    }
}

fn fixture_case_from_table(
    case: &toml::map::Map<String, toml::Value>,
    kind: &str,
    idx: usize,
) -> Result<accuracy_labels::CalibrationFixtureCase, String> {
    let surface_goldens = parse_surface_goldens(case, idx)?;
    Ok(accuracy_labels::CalibrationFixtureCase {
        kind: kind.to_string(),
        expected_cards: required_case_usize(case, "expected_cards", idx)?,
        expected_class: optional_case_string(case, "expected_class", idx)?.map(str::to_string),
        expected_operation_family: optional_case_string(case, "expected_operation_family", idx)?
            .map(str::to_string),
        expected_hazard: optional_case_string(case, "expected_hazard", idx)?.map(str::to_string),
        surface_goldens,
    })
}

/// Parse the optional `surface_goldens` array from a calibration case table.
///
/// Returns an empty `Vec` when the field is absent (most fixtures do not have goldens).
/// Returns an error if the field is present but not an array of non-empty strings,
/// or if any value is not one of the known surface names.
fn parse_surface_goldens(
    case: &toml::map::Map<String, toml::Value>,
    idx: usize,
) -> Result<Vec<String>, String> {
    const KNOWN_SURFACES: &[&str] = &["lsp", "repair-queue", "comment-plan"];
    let Some(value) = case.get("surface_goldens") else {
        return Ok(Vec::new());
    };
    let arr = value.as_array().ok_or_else(|| {
        format!("fixtures/calibration.toml cases[{idx}] `surface_goldens` must be an array")
    })?;
    let mut result = Vec::with_capacity(arr.len());
    for (arr_idx, item) in arr.iter().enumerate() {
        let s = item.as_str().ok_or_else(|| {
            format!(
                "fixtures/calibration.toml cases[{idx}] `surface_goldens`[{arr_idx}] must be a string"
            )
        })?;
        if s.trim().is_empty() {
            return Err(format!(
                "fixtures/calibration.toml cases[{idx}] `surface_goldens`[{arr_idx}] must not be empty"
            ));
        }
        if !KNOWN_SURFACES.contains(&s) {
            return Err(format!(
                "fixtures/calibration.toml cases[{idx}] `surface_goldens`[{arr_idx}] `{s}` is not a known surface; expected one of: {}",
                KNOWN_SURFACES.join(", ")
            ));
        }
        result.push(s.to_string());
    }
    Ok(result)
}

fn index_operation_family(
    case: &toml::map::Map<String, toml::Value>,
    idx: usize,
    fixture: &str,
    index: &mut CalibrationCaseIndex,
) -> Result<(), String> {
    let Some(operation_family) = optional_case_string(case, "expected_operation_family", idx)?
    else {
        return Ok(());
    };
    index
        .operation_families
        .insert(operation_family.to_string());
    index
        .operation_family_fixtures
        .entry(operation_family.to_string())
        .or_default()
        .insert(fixture.to_string());
    Ok(())
}

fn require_all_calibration_kinds(kinds: &BTreeSet<String>) -> Result<(), String> {
    for kind in CALIBRATION_REQUIRED_KINDS {
        if !kinds.contains(*kind) {
            return Err(format!(
                "{CALIBRATION_MANIFEST} is missing a `{kind}` calibration case"
            ));
        }
    }
    Ok(())
}

/// #1712: validates required_core_fixtures parity, also allowing
/// sidecar-registered fixtures (which are auto-required without being listed
/// in the monolith's required_core_fixtures).
fn require_required_core_fixture_parity_with_sidecars(
    required: &[toml::Value],
    fixtures: &BTreeSet<String>,
    sidecar_fixtures: &BTreeSet<String>,
) -> Result<(), String> {
    let required_fixtures = collect_required_core_fixtures(required, fixtures)?;
    for fixture in fixtures {
        if !required_fixtures.contains(fixture) && !sidecar_fixtures.contains(fixture) {
            return Err(format!(
                "{CALIBRATION_MANIFEST} case fixture `{fixture}` is missing from required_core_fixtures"
            ));
        }
    }
    Ok(())
}

fn collect_required_core_fixtures(
    required: &[toml::Value],
    fixtures: &BTreeSet<String>,
) -> Result<BTreeSet<String>, String> {
    let mut required_fixtures = BTreeSet::new();
    for (idx, fixture) in required.iter().enumerate() {
        let Some(fixture) = fixture.as_str() else {
            return Err(format!(
                "{CALIBRATION_MANIFEST} required_core_fixtures[{idx}] must be a string"
            ));
        };
        if !required_fixtures.insert(fixture.to_string()) {
            return Err(format!(
                "{CALIBRATION_MANIFEST} contains duplicate required core fixture `{fixture}`"
            ));
        }
        if !fixtures.contains(fixture) {
            return Err(format!(
                "{CALIBRATION_MANIFEST} required core fixture `{fixture}` has no case"
            ));
        }
    }
    Ok(required_fixtures)
}

fn require_expected_card_fixture_coverage(fixtures: &BTreeSet<String>) -> Result<(), String> {
    for dir in fixture_dirs(&workspace_path("fixtures"))? {
        let fixture = fixture_dir_name(&dir)?;
        if FIXTURE_EXPECTED_CARDS_EXCEPTIONS.contains(&fixture) {
            continue;
        }
        if dir.join("expected.cards.json").is_file() && !fixtures.contains(fixture) {
            return Err(format!(
                "fixture `{fixture}` has expected.cards.json but no {CALIBRATION_MANIFEST} case"
            ));
        }
    }
    Ok(())
}
