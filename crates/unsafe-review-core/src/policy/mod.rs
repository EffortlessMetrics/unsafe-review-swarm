use crate::domain::CardId;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

/// Per-card coverage state recorded in the baseline snapshot (SPEC-0030).
///
/// Stores the three-level coverage state for each slot at the time `baseline init` was run.
/// Used by the `worsened` detector to compare current coverage against the floor.
///
/// Ordinal: `Present`(2) > `Weak`(1) > `Missing`(0).  A card is worsened when ANY
/// slot has a strictly lower ordinal than the snapshot value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SnapshotCoverage {
    /// "present", "weak", or "missing"
    pub(crate) contract_coverage: String,
    /// "present", "weak", or "missing"
    pub(crate) guard_coverage: String,
    /// "present", "weak", or "missing"
    pub(crate) test_reach_coverage: String,
    /// "present" or "missing"
    pub(crate) witness_receipt_coverage: String,
}

impl SnapshotCoverage {
    /// Coverage level ordinal: present=2, weak=1, anything else=0.
    fn ordinal(slot: &str) -> u8 {
        match slot {
            "present" => 2,
            "weak" => 1,
            _ => 0,
        }
    }

    /// Returns `true` when `current` coverage is strictly worse than this snapshot on any slot.
    ///
    /// "Worse" means a slot moved to a lower ordinal (present→weak, present→missing, or
    /// weak→missing).  The witness_receipt slot uses binary present/missing; any downgrade counts.
    pub(crate) fn is_worsened_by(&self, current: &SnapshotCoverage) -> bool {
        Self::ordinal(&current.contract_coverage) < Self::ordinal(&self.contract_coverage)
            || Self::ordinal(&current.guard_coverage) < Self::ordinal(&self.guard_coverage)
            || Self::ordinal(&current.test_reach_coverage)
                < Self::ordinal(&self.test_reach_coverage)
            || Self::ordinal(&current.witness_receipt_coverage)
                < Self::ordinal(&self.witness_receipt_coverage)
    }

    /// Returns `true` when `current` coverage is a **pure improvement** over this snapshot.
    ///
    /// A pure improvement means at least one slot has a strictly higher ordinal than the
    /// snapshot value AND no slot has a lower ordinal.  A mixed up-and-down movement is not
    /// "improved" — it is "worsened" by precedence, so callers must check `is_worsened_by`
    /// before calling this method.
    ///
    /// Precedence rule (caller-enforced; see `summarize`):
    /// worsened > improved > inherited.  If any slot regressed it is worsened; if no slot
    /// regressed but at least one improved it is improved; otherwise it is inherited/unchanged.
    ///
    /// This is a coverage-evidence improvement — the card is still advisory, still open, and
    /// the site is still present.  An improved card is NOT resolved, NOT safe, NOT UB-free,
    /// NOT Miri-clean, and NOT a site-execution claim.
    pub(crate) fn is_improved_by(&self, current: &SnapshotCoverage) -> bool {
        let any_higher = Self::ordinal(&current.contract_coverage)
            > Self::ordinal(&self.contract_coverage)
            || Self::ordinal(&current.guard_coverage) > Self::ordinal(&self.guard_coverage)
            || Self::ordinal(&current.test_reach_coverage)
                > Self::ordinal(&self.test_reach_coverage)
            || Self::ordinal(&current.witness_receipt_coverage)
                > Self::ordinal(&self.witness_receipt_coverage);
        any_higher && !self.is_worsened_by(current)
    }
}

#[derive(Clone, Debug, Default)]
pub(crate) struct PolicyState {
    baseline_ids: BTreeSet<String>,
    suppression_ids: BTreeSet<String>,
    /// Coverage snapshot loaded from `policy/unsafe-review-baseline-snapshot.toml`.
    /// Empty when the snapshot file does not exist.
    pub(crate) coverage_snapshot: BTreeMap<String, SnapshotCoverage>,
}

impl PolicyState {
    pub(crate) fn load(root: &Path) -> Result<Self, String> {
        let policy_dir = root.join("policy");
        Ok(Self {
            baseline_ids: load_ledger_ids(
                &policy_dir.join("unsafe-review-baseline.toml"),
                LedgerKind::Baseline,
            )?,
            suppression_ids: load_ledger_ids(
                &policy_dir.join("unsafe-review-suppressions.toml"),
                LedgerKind::Suppression,
            )?,
            coverage_snapshot: load_coverage_snapshot(
                &policy_dir.join("unsafe-review-baseline-snapshot.toml"),
            )?,
        })
    }

    pub(crate) fn is_baseline_known(&self, id: &CardId) -> bool {
        self.baseline_ids.contains(&id.0)
    }

    pub(crate) fn is_suppressed(&self, id: &CardId) -> bool {
        self.suppression_ids.contains(&id.0)
    }

    /// All card IDs registered in the baseline ledger (for movement computation, SPEC-0030).
    pub(crate) fn baseline_ids(&self) -> &BTreeSet<String> {
        &self.baseline_ids
    }

    /// Snapshot coverage for a specific card ID (for worsened detection, SPEC-0030).
    pub(crate) fn snapshot_for(&self, id: &str) -> Option<&SnapshotCoverage> {
        self.coverage_snapshot.get(id)
    }
}

#[derive(Clone, Copy)]
pub(crate) enum LedgerKind {
    Baseline,
    Suppression,
}

impl LedgerKind {
    fn name(self) -> &'static str {
        match self {
            Self::Baseline => "baseline",
            Self::Suppression => "suppression",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LedgerEntry {
    pub(crate) card_id: String,
    pub(crate) owner: String,
    pub(crate) reason: String,
    pub(crate) evidence: String,
    pub(crate) review_after: Option<String>,
    pub(crate) expires: Option<String>,
}

fn load_ledger_ids(path: &Path, kind: LedgerKind) -> Result<BTreeSet<String>, String> {
    Ok(load_ledger_entries(path, kind)?
        .into_iter()
        .map(|entry| entry.card_id)
        .collect())
}

pub(crate) fn load_ledger_entries(
    path: &Path,
    kind: LedgerKind,
) -> Result<Vec<LedgerEntry>, String> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    let value = text
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("{} is not valid TOML: {err}", path.display()))?;
    let status = value
        .get("status")
        .and_then(toml::Value::as_str)
        .unwrap_or("active");
    let entries = value
        .get("entries")
        .and_then(toml::Value::as_array)
        .map_or(&[][..], Vec::as_slice);

    if status == "empty" {
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        return Err(format!(
            "{} status is empty but has entries",
            path.display()
        ));
    }

    let mut records = Vec::new();
    for (idx, entry) in entries.iter().enumerate() {
        let Some(entry) = entry.as_table() else {
            return Err(format!("{} entries[{idx}] must be a table", path.display()));
        };
        let card_id = required_string(entry, "card_id", path, idx)?;
        let owner = required_string(entry, "owner", path, idx)?;
        let reason = required_string(entry, "reason", path, idx)?;
        let evidence = required_string(entry, "evidence", path, idx)?;
        let has_review_after = optional_date(entry, "review_after", path, idx)?;
        let has_expires = optional_date(entry, "expires", path, idx)?;
        match kind {
            LedgerKind::Baseline if !has_review_after => {
                return Err(format!(
                    "{} entries[{idx}] baseline entry is missing review_after",
                    path.display()
                ));
            }
            LedgerKind::Suppression if !has_review_after && !has_expires => {
                return Err(format!(
                    "{} entries[{idx}] suppression entry must set review_after or expires",
                    path.display()
                ));
            }
            _ => {}
        }
        if !looks_like_counted_card_id(card_id) {
            return Err(format!(
                "{} entries[{idx}] {} card_id must be an exact counted UR-* identity ending in -cN",
                path.display(),
                kind.name()
            ));
        }
        records.push(LedgerEntry {
            card_id: card_id.to_string(),
            owner: owner.to_string(),
            reason: reason.to_string(),
            evidence: evidence.to_string(),
            review_after: optional_string(entry, "review_after"),
            expires: match kind {
                LedgerKind::Baseline => None,
                LedgerKind::Suppression => optional_string(entry, "expires"),
            },
        });
    }

    Ok(records)
}

fn required_string<'a>(
    entry: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &Path,
    idx: usize,
) -> Result<&'a str, String> {
    let Some(value) = entry.get(key).and_then(toml::Value::as_str) else {
        return Err(format!(
            "{} entries[{idx}] is missing string `{key}`",
            path.display()
        ));
    };
    if value.trim().is_empty() {
        Err(format!(
            "{} entries[{idx}] string `{key}` is empty",
            path.display()
        ))
    } else {
        Ok(value)
    }
}

fn optional_date(
    entry: &toml::map::Map<String, toml::Value>,
    key: &str,
    path: &Path,
    idx: usize,
) -> Result<bool, String> {
    let Some(value) = entry.get(key) else {
        return Ok(false);
    };
    let Some(value) = value.as_str() else {
        return Err(format!(
            "{} entries[{idx}] `{key}` must be a string",
            path.display()
        ));
    };
    if !looks_like_iso_date(value) {
        return Err(format!(
            "{} entries[{idx}] `{key}` must use YYYY-MM-DD",
            path.display()
        ));
    }
    Ok(true)
}

fn optional_string(entry: &toml::map::Map<String, toml::Value>, key: &str) -> Option<String> {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToOwned::to_owned)
}

fn looks_like_iso_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

fn looks_like_counted_card_id(value: &str) -> bool {
    let Some((prefix, count)) = value.rsplit_once("-c") else {
        return false;
    };
    value.starts_with("UR-")
        && !prefix.is_empty()
        && !count.is_empty()
        && count.bytes().all(|byte| byte.is_ascii_digit())
}

/// Load the coverage snapshot from `policy/unsafe-review-baseline-snapshot.toml`.
///
/// The snapshot file uses:
/// ```toml
/// schema_version = "0.1"
/// policy = "unsafe-review-baseline-snapshot"
///
/// [[entries]]
/// card_id = "UR-...-c1"
/// contract_coverage = "present"   # present | weak | missing
/// guard_coverage = "missing"
/// test_reach_coverage = "missing"
/// witness_receipt_coverage = "missing"  # present | missing
/// ```
///
/// Missing or non-existent snapshot → empty map (no worsened detection).
pub(crate) fn load_coverage_snapshot(
    path: &Path,
) -> Result<BTreeMap<String, SnapshotCoverage>, String> {
    if !path.is_file() {
        return Ok(BTreeMap::new());
    }
    let text =
        fs::read_to_string(path).map_err(|err| format!("read {} failed: {err}", path.display()))?;
    let value = text
        .parse::<toml::Table>()
        .map(toml::Value::Table)
        .map_err(|err| format!("{} is not valid TOML: {err}", path.display()))?;
    let entries = value
        .get("entries")
        .and_then(toml::Value::as_array)
        .map_or(&[][..], Vec::as_slice);
    let mut map = BTreeMap::new();
    for (idx, entry) in entries.iter().enumerate() {
        let Some(entry) = entry.as_table() else {
            return Err(format!("{} entries[{idx}] must be a table", path.display()));
        };
        let card_id = snapshot_required_string(entry, "card_id", path, idx)?.to_string();
        let contract_coverage =
            snapshot_required_string(entry, "contract_coverage", path, idx)?.to_string();
        let guard_coverage =
            snapshot_required_string(entry, "guard_coverage", path, idx)?.to_string();
        let test_reach_coverage =
            snapshot_required_string(entry, "test_reach_coverage", path, idx)?.to_string();
        let witness_receipt_coverage =
            snapshot_required_string(entry, "witness_receipt_coverage", path, idx)?.to_string();
        map.insert(
            card_id,
            SnapshotCoverage {
                contract_coverage,
                guard_coverage,
                test_reach_coverage,
                witness_receipt_coverage,
            },
        );
    }
    Ok(map)
}

/// Write a coverage snapshot file deterministically (sorted by card_id, LF, UTF-8 no-BOM).
pub(crate) fn write_coverage_snapshot(
    path: &Path,
    entries: &BTreeMap<String, SnapshotCoverage>,
) -> Result<(), String> {
    let mut text = String::new();
    text.push_str("schema_version = \"0.1\"\n");
    text.push_str("policy = \"unsafe-review-baseline-snapshot\"\n");
    text.push('\n');
    for (card_id, cov) in entries {
        text.push_str("[[entries]]\n");
        text.push_str(&format!("card_id = \"{card_id}\"\n"));
        text.push_str(&format!(
            "contract_coverage = \"{}\"\n",
            cov.contract_coverage
        ));
        text.push_str(&format!("guard_coverage = \"{}\"\n", cov.guard_coverage));
        text.push_str(&format!(
            "test_reach_coverage = \"{}\"\n",
            cov.test_reach_coverage
        ));
        text.push_str(&format!(
            "witness_receipt_coverage = \"{}\"\n",
            cov.witness_receipt_coverage
        ));
        text.push('\n');
    }
    // Ensure LF line endings regardless of platform.
    let lf_text = text.replace("\r\n", "\n");
    ensure_parent_dir_exists(path)?;
    fs::write(path, lf_text.as_bytes())
        .map_err(|err| format!("write {} failed: {err}", path.display()))
}

/// Write the baseline ledger file deterministically (sorted by card_id, LF, UTF-8 no-BOM).
///
/// If the path already exists its entries are merged (existing wins; new entries are added).
pub(crate) fn merge_and_write_baseline_ledger(
    path: &Path,
    new_entries: &[LedgerEntry],
) -> Result<(), String> {
    // Load existing entries (may be empty if file is new).
    let mut existing = load_ledger_entries(path, LedgerKind::Baseline).unwrap_or_default();
    // Add or update: new_entries win for the same card_id.
    for new in new_entries {
        if let Some(pos) = existing.iter().position(|e| e.card_id == new.card_id) {
            existing[pos] = new.clone();
        } else {
            existing.push(new.clone());
        }
    }
    // Sort by card_id for determinism.
    existing.sort_by(|a, b| a.card_id.cmp(&b.card_id));

    let mut text = String::new();
    text.push_str("schema_version = \"0.1\"\n");
    text.push_str("policy = \"unsafe-review-baseline\"\n");
    text.push_str("owner = \"baseline-init\"\n");
    if existing.is_empty() {
        text.push_str("status = \"empty\"\n");
    } else {
        text.push_str("status = \"active\"\n");
    }
    text.push('\n');
    for entry in &existing {
        text.push_str("[[entries]]\n");
        text.push_str(&format!("card_id = \"{}\"\n", entry.card_id));
        text.push_str(&format!("owner = \"{}\"\n", entry.owner));
        text.push_str(&format!("reason = \"{}\"\n", entry.reason));
        text.push_str(&format!("evidence = \"{}\"\n", entry.evidence));
        if let Some(review_after) = &entry.review_after {
            text.push_str(&format!("review_after = \"{review_after}\"\n"));
        }
        text.push('\n');
    }
    let lf_text = text.replace("\r\n", "\n");
    ensure_parent_dir_exists(path)?;
    fs::write(path, lf_text.as_bytes())
        .map_err(|err| format!("write {} failed: {err}", path.display()))
}

fn ensure_parent_dir_exists(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    Ok(())
}

fn snapshot_required_string<'a>(
    entry: &'a toml::map::Map<String, toml::Value>,
    key: &str,
    path: &Path,
    idx: usize,
) -> Result<&'a str, String> {
    entry
        .get(key)
        .and_then(toml::Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| {
            format!(
                "{} entries[{idx}] is missing or empty string `{key}`",
                path.display()
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn missing_policy_files_load_empty_state() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-missing-policy")?;
        fs::create_dir_all(&root).map_err(|err| format!("create temp root failed: {err}"))?;

        let state = PolicyState::load(&root)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert!(!state.is_baseline_known(&CardId("UR-example-c1".to_string())));
        assert!(!state.is_suppressed(&CardId("UR-example-c1".to_string())));
        Ok(())
    }

    #[test]
    fn policy_state_loads_baseline_and_suppression_ids() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-policy-state")?;
        let policy = root.join("policy");
        fs::create_dir_all(&policy).map_err(|err| format!("create policy dir failed: {err}"))?;
        let baseline =
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1";
        let suppression =
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c2";
        fs::write(
            policy.join("unsafe-review-baseline.toml"),
            format!(
                r#"schema_version = "0.1"
status = "active"

[[entries]]
card_id = "{baseline}"
owner = "core/policy"
reason = "accepted current debt"
evidence = "fixture"
review_after = "2026-08-01"
"#
            ),
        )
        .map_err(|err| format!("write baseline failed: {err}"))?;
        fs::write(
            policy.join("unsafe-review-suppressions.toml"),
            format!(
                r#"schema_version = "0.1"
status = "active"

[[entries]]
card_id = "{suppression}"
owner = "core/policy"
reason = "false positive"
evidence = "fixture"
expires = "2026-08-01"
"#
            ),
        )
        .map_err(|err| format!("write suppressions failed: {err}"))?;

        let state = PolicyState::load(&root)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;
        assert!(state.is_baseline_known(&CardId(baseline.to_string())));
        assert!(state.is_suppressed(&CardId(suppression.to_string())));
        Ok(())
    }

    // ── SnapshotCoverage::is_worsened_by ──────────────────────────────────────

    fn snap(contract: &str, guard: &str, test_reach: &str, witness: &str) -> SnapshotCoverage {
        SnapshotCoverage {
            contract_coverage: contract.to_string(),
            guard_coverage: guard.to_string(),
            test_reach_coverage: test_reach.to_string(),
            witness_receipt_coverage: witness.to_string(),
        }
    }

    #[test]
    fn worsened_detected_when_contract_regresses() {
        // Baseline had present; current is weak → worsened.
        let baseline = snap("present", "present", "present", "missing");
        let current = snap("weak", "present", "present", "missing");
        assert!(
            baseline.is_worsened_by(&current),
            "present→weak on contract must be detected as worsened"
        );
    }

    #[test]
    fn worsened_detected_when_guard_regresses_to_missing() {
        // Baseline had weak; current is missing → worsened.
        let baseline = snap("present", "weak", "present", "missing");
        let current = snap("present", "missing", "present", "missing");
        assert!(
            baseline.is_worsened_by(&current),
            "weak→missing on guard must be detected as worsened"
        );
    }

    #[test]
    fn worsened_detected_when_witness_receipt_regresses() {
        // Baseline had present; current is missing → worsened on witness slot.
        let baseline = snap("present", "present", "present", "present");
        let current = snap("present", "present", "present", "missing");
        assert!(
            baseline.is_worsened_by(&current),
            "present→missing on witness_receipt must be detected as worsened"
        );
    }

    #[test]
    fn not_worsened_when_coverage_unchanged() {
        // Inherited-unchanged card: same snapshot as baseline → NOT worsened.
        let baseline = snap("present", "weak", "missing", "missing");
        let current = snap("present", "weak", "missing", "missing");
        assert!(
            !baseline.is_worsened_by(&current),
            "identical coverage must NOT be counted as worsened (inherited-unchanged card)"
        );
    }

    #[test]
    fn not_worsened_when_coverage_improves() {
        // Coverage improved: weak→present on guard → NOT worsened.
        let baseline = snap("weak", "weak", "missing", "missing");
        let current = snap("present", "present", "missing", "missing");
        assert!(
            !baseline.is_worsened_by(&current),
            "coverage improvement must NOT be counted as worsened"
        );
    }

    // ── SnapshotCoverage::is_improved_by ──────────────────────────────────────

    #[test]
    fn improved_detected_when_contract_advances_from_missing_to_present() {
        // Baseline had missing contract; current adds a SAFETY doc → improved.
        let baseline = snap("missing", "missing", "missing", "missing");
        let current = snap("present", "missing", "missing", "missing");
        assert!(
            baseline.is_improved_by(&current),
            "missing→present on contract must be detected as improved"
        );
    }

    #[test]
    fn improved_detected_when_guard_advances_from_weak_to_present() {
        // Baseline had weak guard; current strengthens it → improved.
        let baseline = snap("missing", "weak", "missing", "missing");
        let current = snap("missing", "present", "missing", "missing");
        assert!(
            baseline.is_improved_by(&current),
            "weak→present on guard must be detected as improved"
        );
    }

    #[test]
    fn not_improved_when_coverage_unchanged() {
        // Inherited-unchanged card: same snapshot → NOT improved.
        let baseline = snap("present", "weak", "missing", "missing");
        let current = snap("present", "weak", "missing", "missing");
        assert!(
            !baseline.is_improved_by(&current),
            "identical coverage must NOT be counted as improved (inherited-unchanged card)"
        );
    }

    #[test]
    fn not_improved_when_coverage_worsens() {
        // A slot regressed → worsened wins, not improved.
        let baseline = snap("present", "present", "present", "missing");
        let current = snap("weak", "present", "present", "missing");
        assert!(
            !baseline.is_improved_by(&current),
            "regression on any slot must NOT be counted as improved (worsened wins)"
        );
    }

    #[test]
    fn not_improved_when_mixed_up_and_down() {
        // One slot improved (test_reach: missing→present) but another regressed
        // (guard: present→weak) → mixed: worsened wins, NOT improved.
        let baseline = snap("missing", "present", "missing", "missing");
        let current = snap("missing", "weak", "present", "missing");
        assert!(
            !baseline.is_improved_by(&current),
            "mixed up-and-down movement must NOT be counted as improved (worsened takes precedence)"
        );
    }

    #[test]
    fn improved_when_multiple_slots_advance_and_none_regress() {
        // Both contract and guard improved → improved.
        let baseline = snap("missing", "missing", "missing", "missing");
        let current = snap("present", "weak", "missing", "missing");
        assert!(
            baseline.is_improved_by(&current),
            "multiple slot advances with no regression must be detected as improved"
        );
    }

    // ── write_coverage_snapshot + load_coverage_snapshot round-trip ──────────

    #[test]
    fn snapshot_round_trip() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-snapshot-rt")?;
        let policy = root.join("policy");
        fs::create_dir_all(&policy).map_err(|err| format!("create policy dir failed: {err}"))?;
        let path = policy.join("unsafe-review-baseline-snapshot.toml");

        let card_a = "UR-crate-src-lib-rs-op-raw_ptr-c1";
        let card_b = "UR-crate-src-lib-rs-op-raw_ptr-c2";
        let mut entries = BTreeMap::new();
        entries.insert(
            card_a.to_string(),
            snap("present", "weak", "missing", "missing"),
        );
        entries.insert(
            card_b.to_string(),
            snap("missing", "missing", "missing", "present"),
        );

        write_coverage_snapshot(&path, &entries)?;
        let loaded = load_coverage_snapshot(&path)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp root failed: {err}"))?;

        assert_eq!(loaded.len(), 2, "should have loaded 2 entries");
        let a = loaded
            .get(card_a)
            .ok_or("card_a missing from loaded snapshot")?;
        assert_eq!(a.contract_coverage, "present");
        assert_eq!(a.guard_coverage, "weak");
        assert_eq!(a.test_reach_coverage, "missing");
        assert_eq!(a.witness_receipt_coverage, "missing");

        let b = loaded
            .get(card_b)
            .ok_or("card_b missing from loaded snapshot")?;
        assert_eq!(b.witness_receipt_coverage, "present");
        Ok(())
    }

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }
}
