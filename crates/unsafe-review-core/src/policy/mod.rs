use crate::domain::CardId;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub(crate) struct PolicyState {
    baseline_ids: BTreeSet<String>,
    suppression_ids: BTreeSet<String>,
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

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }
}
