//! Manifest-only corpus partition validation.
//!
//! Partitions are an overlay on the existing corpus ledgers, not a new ledger.
//! This check proves each declared corpus case resolves to exactly one of the
//! accepted partition owners and rejects branch-like floating refs before they
//! become release or tuning inputs.

use std::collections::BTreeMap;

use crate::{parse_toml_file, workspace_path};

const CALIBRATION_MANIFEST: &str = "fixtures/calibration.toml";
const DOGFOOD_MANIFEST: &str = "docs/dogfood/corpus.toml";
const PR_CORPUS_LEDGER: &str = "policy/pr-corpus.toml";
const EVIDENCE_LOSS_CHALLENGES_LEDGER: &str = "policy/evidence-loss-challenges.toml";

const PARTITIONS: &[&str] = &["conformance", "regression", "holdout"];
const FLOATING_REF_KEYS: &[&str] = &[
    "ref",
    "branch",
    "base_ref",
    "head_ref",
    "base_branch",
    "head_branch",
    "commit_ref",
    "revision",
];
const SHA_KEYS: &[&str] = &["commit", "base_sha", "head_sha"];

#[derive(Default)]
struct PartitionStats {
    total: usize,
    conformance: usize,
    regression: usize,
    holdout: usize,
}

impl PartitionStats {
    fn record(&mut self, partition: &str) {
        self.total += 1;
        match partition {
            "conformance" => self.conformance += 1,
            "regression" => self.regression += 1,
            "holdout" => self.holdout += 1,
            _ => {}
        }
    }
}

pub(crate) fn check() -> Result<(), String> {
    let mut stats = PartitionStats::default();

    check_calibration_manifest(&parse_manifest(CALIBRATION_MANIFEST)?, &mut stats)?;
    check_dogfood_manifest(&parse_manifest(DOGFOOD_MANIFEST)?, &mut stats)?;
    check_pr_corpus_manifest(&parse_manifest(PR_CORPUS_LEDGER)?, &mut stats)?;
    check_evidence_loss_challenges_manifest(
        &parse_manifest(EVIDENCE_LOSS_CHALLENGES_LEDGER)?,
        &mut stats,
    )?;

    if stats.total == 0 {
        return Err("corpus partition check found no corpus cases".to_string());
    }

    println!(
        "check-corpus-partitions: ok ({} cases: conformance={}, regression={}, holdout={})",
        stats.total, stats.conformance, stats.regression, stats.holdout
    );
    Ok(())
}

fn parse_manifest(path: &str) -> Result<toml::Value, String> {
    parse_toml_file(&workspace_path(path))
}

fn check_calibration_manifest(
    manifest: &toml::Value,
    stats: &mut PartitionStats,
) -> Result<(), String> {
    let default = partition_default(manifest, CALIBRATION_MANIFEST)?;
    require_partition_value(
        default,
        "conformance",
        &format!("{CALIBRATION_MANIFEST} partition_default"),
    )?;
    let cases = array_at(manifest, CALIBRATION_MANIFEST, "cases")?;
    let by_kind = BTreeMap::new();
    for (idx, case) in cases.iter().enumerate() {
        let table = table_at(case, CALIBRATION_MANIFEST, "cases", idx)?;
        let context = format!("{CALIBRATION_MANIFEST} cases[{idx}]");
        let partition = resolve_partition(table, &by_kind, Some(default), "case", &context)?;
        reject_floating_ref_keys(table, &context)?;
        reject_every_pr_holdout(table, &context, partition)?;
        stats.record(partition);
    }
    Ok(())
}

fn check_dogfood_manifest(
    manifest: &toml::Value,
    stats: &mut PartitionStats,
) -> Result<(), String> {
    let by_kind = partition_by_kind(manifest, DOGFOOD_MANIFEST)?;
    require_kind_partition(&by_kind, DOGFOOD_MANIFEST, "fixture-control", "conformance")?;
    require_kind_partition(&by_kind, DOGFOOD_MANIFEST, "repo-snapshot", "regression")?;
    require_kind_partition(&by_kind, DOGFOOD_MANIFEST, "pr-diff", "regression")?;
    let targets = array_at(manifest, DOGFOOD_MANIFEST, "targets")?;
    for (idx, target) in targets.iter().enumerate() {
        let table = table_at(target, DOGFOOD_MANIFEST, "targets", idx)?;
        let kind = required_table_string(table, DOGFOOD_MANIFEST, "targets", idx, "kind")?;
        let context = format!("{DOGFOOD_MANIFEST} targets[{idx}] ({kind})");
        let partition = resolve_partition(table, &by_kind, None, kind, &context)?;
        reject_floating_ref_keys(table, &context)?;
        validate_sha_fields(table, &context)?;
        reject_every_pr_holdout(table, &context, partition)?;
        stats.record(partition);
    }
    Ok(())
}

fn check_pr_corpus_manifest(
    manifest: &toml::Value,
    stats: &mut PartitionStats,
) -> Result<(), String> {
    let by_kind = partition_by_kind(manifest, PR_CORPUS_LEDGER)?;
    require_kind_partition(
        &by_kind,
        PR_CORPUS_LEDGER,
        "synthetic-fixture",
        "conformance",
    )?;
    let cases = array_at(manifest, PR_CORPUS_LEDGER, "pr")?;
    for (idx, case) in cases.iter().enumerate() {
        let table = table_at(case, PR_CORPUS_LEDGER, "pr", idx)?;
        let kind = required_table_string(table, PR_CORPUS_LEDGER, "pr", idx, "kind")?;
        let context = format!("{PR_CORPUS_LEDGER} pr[{idx}] ({kind})");
        let partition = resolve_partition(table, &by_kind, None, kind, &context)?;
        reject_floating_ref_keys(table, &context)?;
        validate_sha_fields(table, &context)?;
        reject_every_pr_holdout(table, &context, partition)?;
        stats.record(partition);
    }
    Ok(())
}

fn check_evidence_loss_challenges_manifest(
    manifest: &toml::Value,
    stats: &mut PartitionStats,
) -> Result<(), String> {
    let by_kind = partition_by_kind(manifest, EVIDENCE_LOSS_CHALLENGES_LEDGER)?;
    require_kind_partition(
        &by_kind,
        EVIDENCE_LOSS_CHALLENGES_LEDGER,
        "fixture-transform",
        "conformance",
    )?;
    let cases = array_at(manifest, EVIDENCE_LOSS_CHALLENGES_LEDGER, "challenge")?;
    for (idx, case) in cases.iter().enumerate() {
        let table = table_at(case, EVIDENCE_LOSS_CHALLENGES_LEDGER, "challenge", idx)?;
        let kind = required_table_string(
            table,
            EVIDENCE_LOSS_CHALLENGES_LEDGER,
            "challenge",
            idx,
            "kind",
        )?;
        let context = format!("{EVIDENCE_LOSS_CHALLENGES_LEDGER} challenge[{idx}] ({kind})");
        let source_fixture = required_table_string(
            table,
            EVIDENCE_LOSS_CHALLENGES_LEDGER,
            "challenge",
            idx,
            "source_fixture",
        )?;
        if !source_fixture.starts_with("fixtures/") {
            return Err(format!(
                "{context} source_fixture must live under fixtures/"
            ));
        }
        let partition = resolve_partition(table, &by_kind, None, kind, &context)?;
        reject_floating_ref_keys(table, &context)?;
        validate_sha_fields(table, &context)?;
        reject_every_pr_holdout(table, &context, partition)?;
        stats.record(partition);
    }
    Ok(())
}

fn partition_default<'a>(manifest: &'a toml::Value, path: &str) -> Result<&'a str, String> {
    let default = manifest
        .get("partition_default")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{path} is missing string `partition_default`"))?;
    validate_partition(default, &format!("{path} partition_default"))?;
    Ok(default)
}

fn partition_by_kind(
    manifest: &toml::Value,
    path: &str,
) -> Result<BTreeMap<String, String>, String> {
    let table = manifest
        .get("partition_by_kind")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{path} is missing table `partition_by_kind`"))?;
    if table.is_empty() {
        return Err(format!("{path} partition_by_kind must not be empty"));
    }

    let mut partitions = BTreeMap::new();
    for (kind, value) in table {
        let partition = value
            .as_str()
            .ok_or_else(|| format!("{path} partition_by_kind.{kind} must be a string"))?;
        validate_partition(partition, &format!("{path} partition_by_kind.{kind}"))?;
        partitions.insert(kind.to_string(), partition.to_string());
    }
    Ok(partitions)
}

fn resolve_partition<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    by_kind: &'a BTreeMap<String, String>,
    default: Option<&'a str>,
    kind: &str,
    context: &str,
) -> Result<&'a str, String> {
    if let Some(partition) = table.get("partition").and_then(toml::Value::as_str) {
        validate_partition(partition, &format!("{context} partition"))?;
        return Ok(partition);
    }
    if let Some(partition) = by_kind.get(kind) {
        return Ok(partition.as_str());
    }
    if let Some(partition) = default {
        return Ok(partition);
    }
    Err(format!(
        "{context} has no partition owner; add `partition` or a matching `partition_by_kind` entry"
    ))
}

fn validate_partition(partition: &str, context: &str) -> Result<(), String> {
    if PARTITIONS.contains(&partition) {
        Ok(())
    } else {
        Err(format!(
            "{context} uses unknown partition `{partition}`; expected one of: {}",
            PARTITIONS.join(", ")
        ))
    }
}

fn require_kind_partition(
    by_kind: &BTreeMap<String, String>,
    path: &str,
    kind: &str,
    expected: &str,
) -> Result<(), String> {
    let Some(actual) = by_kind.get(kind) else {
        return Err(format!(
            "{path} partition_by_kind is missing required `{kind}` owner"
        ));
    };
    require_partition_value(
        actual,
        expected,
        &format!("{path} partition_by_kind.{kind}"),
    )
}

fn require_partition_value(actual: &str, expected: &str, context: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "{context} must be `{expected}` for the current corpus cadence, got `{actual}`"
        ))
    }
}

fn reject_floating_ref_keys(
    table: &toml::map::Map<String, toml::Value>,
    context: &str,
) -> Result<(), String> {
    for key in FLOATING_REF_KEYS {
        if table.contains_key(*key) {
            return Err(format!(
                "{context} uses floating ref field `{key}`; pin exact SHAs or checked-in diffs instead"
            ));
        }
    }
    Ok(())
}

fn validate_sha_fields(
    table: &toml::map::Map<String, toml::Value>,
    context: &str,
) -> Result<(), String> {
    for key in SHA_KEYS {
        if let Some(value) = table.get(*key) {
            let sha = value
                .as_str()
                .ok_or_else(|| format!("{context} `{key}` must be a string SHA"))?;
            if !is_full_sha(sha) {
                return Err(format!(
                    "{context} `{key}` must be a full 40-character hex SHA"
                ));
            }
        }
    }
    Ok(())
}

fn is_full_sha(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn reject_every_pr_holdout(
    table: &toml::map::Map<String, toml::Value>,
    context: &str,
    partition: &str,
) -> Result<(), String> {
    if partition != "holdout" {
        return Ok(());
    }
    for key in ["cadence", "run_cadence", "tuning_cadence"] {
        if table.get(key).and_then(toml::Value::as_str) == Some("every-pr") {
            return Err(format!(
                "{context} is partitioned as holdout but sets `{key}` to every-pr"
            ));
        }
    }
    Ok(())
}

fn array_at<'a>(
    manifest: &'a toml::Value,
    path: &str,
    key: &str,
) -> Result<&'a [toml::Value], String> {
    let array = manifest
        .get(key)
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{path} is missing array `{key}`"))?;
    if array.is_empty() {
        return Err(format!("{path} array `{key}` must not be empty"));
    }
    Ok(array)
}

fn table_at<'a>(
    value: &'a toml::Value,
    path: &str,
    key: &str,
    idx: usize,
) -> Result<&'a toml::map::Map<String, toml::Value>, String> {
    value
        .as_table()
        .ok_or_else(|| format!("{path} {key}[{idx}] must be a TOML table"))
}

fn required_table_string<'a>(
    table: &'a toml::map::Map<String, toml::Value>,
    path: &str,
    array_key: &str,
    idx: usize,
    key: &str,
) -> Result<&'a str, String> {
    table
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{path} {array_key}[{idx}] missing non-empty `{key}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<toml::Value, String> {
        text.parse::<toml::Table>()
            .map(toml::Value::Table)
            .map_err(|err| format!("test TOML failed to parse: {err}"))
    }

    #[test]
    fn calibration_requires_partition_default() -> Result<(), String> {
        let manifest = parse(
            r#"
schema_version = "0.1"

[[cases]]
fixture = "raw_pointer_alignment"
"#,
        )?;
        let mut stats = PartitionStats::default();

        let err = check_calibration_manifest(&manifest, &mut stats)
            .err()
            .ok_or_else(|| "expected missing partition_default to fail".to_string())?;

        assert!(err.contains("partition_default"), "{err}");
        Ok(())
    }

    #[test]
    fn calibration_holdout_override_cannot_use_every_pr_cadence() -> Result<(), String> {
        let manifest = parse(
            r#"
schema_version = "0.1"
partition_default = "conformance"

[[cases]]
fixture = "raw_pointer_alignment"
partition = "holdout"
tuning_cadence = "every-pr"
"#,
        )?;
        let mut stats = PartitionStats::default();

        let err = check_calibration_manifest(&manifest, &mut stats)
            .err()
            .ok_or_else(|| "expected calibration holdout every-pr cadence to fail".to_string())?;

        assert!(err.contains("partitioned as holdout"), "{err}");
        Ok(())
    }

    #[test]
    fn dogfood_by_kind_defaults_assign_partitions() -> Result<(), String> {
        let manifest = parse(
            r#"
schema_version = "0.1"
partition_by_kind = { "fixture-control" = "conformance", "repo-snapshot" = "regression", "pr-diff" = "regression" }

[[targets]]
id = "smallvec-capped"
kind = "repo-snapshot"
commit = "bc8a854926a8d940164f6c4ad4fc6efe51962e93"
"#,
        )?;
        let mut stats = PartitionStats::default();

        check_dogfood_manifest(&manifest, &mut stats)?;

        assert_eq!(stats.total, 1);
        assert_eq!(stats.regression, 1);
        Ok(())
    }

    #[test]
    fn dogfood_rejects_floating_ref_fields() -> Result<(), String> {
        let manifest = parse(
            r#"
schema_version = "0.1"
partition_by_kind = { "fixture-control" = "conformance", "repo-snapshot" = "regression", "pr-diff" = "regression" }

[[targets]]
id = "smallvec-capped"
kind = "repo-snapshot"
branch = "main"
"#,
        )?;
        let mut stats = PartitionStats::default();

        let err = check_dogfood_manifest(&manifest, &mut stats)
            .err()
            .ok_or_else(|| "expected floating branch to fail".to_string())?;

        assert!(err.contains("floating ref field `branch`"), "{err}");
        Ok(())
    }

    #[test]
    fn holdout_cannot_use_every_pr_cadence() -> Result<(), String> {
        let manifest = parse(
            r#"
schema_version = "1.0"
partition_by_kind = { "synthetic-fixture" = "conformance" }

[[pr]]
id = "future-holdout"
kind = "synthetic-fixture"
partition = "holdout"
tuning_cadence = "every-pr"
"#,
        )?;
        let mut stats = PartitionStats::default();

        let err = check_pr_corpus_manifest(&manifest, &mut stats)
            .err()
            .ok_or_else(|| "expected every-pr holdout to fail".to_string())?;

        assert!(err.contains("partitioned as holdout"), "{err}");
        Ok(())
    }

    #[test]
    fn evidence_loss_challenges_by_kind_defaults_assign_conformance() -> Result<(), String> {
        let manifest = parse(
            r#"
schema_version = "1.0"
partition_by_kind = { "fixture-transform" = "conformance" }

[[challenge]]
id = "remove-safety-section"
kind = "fixture-transform"
source_fixture = "fixtures/raw_pointer_deref_coverage_improved"
"#,
        )?;
        let mut stats = PartitionStats::default();

        check_evidence_loss_challenges_manifest(&manifest, &mut stats)?;

        assert_eq!(stats.total, 1);
        assert_eq!(stats.conformance, 1);
        Ok(())
    }
}
