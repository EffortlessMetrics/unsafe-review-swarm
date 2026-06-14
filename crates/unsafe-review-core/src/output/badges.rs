use crate::api::{AnalyzeOutput, Summary};
use serde::Serialize;

pub(crate) fn render(output: &AnalyzeOutput) -> (String, String) {
    let base_count = open_actionable_count(&output.summary);
    let base_color = badge_color(base_count);
    let plus_count = evidence_quality_count(&output.summary);
    let plus_color = badge_color(plus_count);
    let main = badge("unsafe-review", base_count, base_color);
    let plus = badge("unsafe-review+", plus_count, plus_color);
    (render_pretty(&main), render_pretty(&plus))
}

/// Return the baseline-aware open-actionable count for the main badge (SPEC-0031).
///
/// When a recorded baseline is present (`inherited_gaps` or `resolved_gaps` are nonzero),
/// the badge shows movement-relevant gaps only: `new_gaps + worsened_gaps`.  This reflects
/// cards that are above the baseline floor, not the full inherited debt.
///
/// When no baseline exists, fall back to the raw `open_actionable_gaps` count — the honest
/// "this repo has not set a floor yet" reading.
fn open_actionable_count(summary: &Summary) -> usize {
    if has_baseline(summary) {
        summary.new_gaps + summary.worsened_gaps
    } else {
        summary.open_actionable_gaps
    }
}

/// Return the baseline-movement-aware evidence-quality count for the `unsafe-review+` badge
/// (SPEC-0031).
///
/// `contract_missing + guard_missing + guarded_unwitnessed` counts only NEW (non-baseline)
/// cards by class — `BaselineKnown` cards fall through the class match and are not
/// incremented in those buckets.  However, `worsened_gaps` are `BaselineKnown` cards whose
/// evidence-quality slots regressed versus the saved snapshot.  A worsened baseline card
/// the main badge counts as a movement-relevant gap must also be reflected in the plus count.
///
/// When a recorded baseline is present, add `worsened_gaps` to the slot-bucket total so the
/// plus badge and the main badge use compatible movement semantics.  When no baseline exists
/// fall back to the raw slot-bucket total (the "no floor set yet" reading).
fn evidence_quality_count(summary: &Summary) -> usize {
    let slot_gaps = summary.contract_missing + summary.guard_missing + summary.guarded_unwitnessed;
    if has_baseline(summary) {
        // `worsened_gaps` are BaselineKnown cards whose evidence-quality regressed; they are
        // not included in the slot-bucket counters above.  Adding them here makes the plus
        // badge movement-compatible with the main badge (SPEC-0031 §counts-are-baseline-aware).
        slot_gaps + summary.worsened_gaps
    } else {
        slot_gaps
    }
}

/// A recorded baseline floor is present when at least one card is inherited (still open,
/// matched the baseline ledger) or at least one baseline entry has since been resolved.
/// Both require a non-empty baseline ledger to be non-zero.
fn has_baseline(summary: &Summary) -> bool {
    summary.inherited_gaps > 0 || summary.resolved_gaps > 0
}

fn badge_color(count: usize) -> &'static str {
    if count == 0 {
        "green"
    } else if count < 10 {
        "yellow"
    } else {
        "orange"
    }
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(mut text) => {
            text.push('\n');
            text
        }
        Err(err) => format!("{{\n  \"error\": \"badge serialization failed: {err}\"\n}}\n"),
    }
}

fn badge(label: &'static str, count: usize, color: &'static str) -> BadgeJson {
    BadgeJson {
        schema_version: 1,
        label,
        message: count.to_string(),
        color,
    }
}

#[derive(Serialize)]
struct BadgeJson {
    #[serde(rename = "schemaVersion")]
    schema_version: u8,
    label: &'static str,
    message: String,
    color: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{
        AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, PolicyMode, Scope, Summary,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    #[test]
    fn badge_json_counts_open_gaps_without_safety_claim() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let (main, plus) = render(&output);
        let main = parse_json(&main)?;
        let plus = parse_json(&plus)?;

        assert_eq!(main["schemaVersion"], 1);
        assert_eq!(main["label"], "unsafe-review");
        assert_eq!(main["message"], "1");
        assert_eq!(main["color"], "yellow");
        assert_shields_endpoint_fields_only(&main)?;
        assert_ne!(main["message"], "safe");

        assert_eq!(plus["schemaVersion"], 1);
        assert_eq!(plus["label"], "unsafe-review+");
        assert_eq!(plus["message"], "1");
        assert_eq!(plus["color"], "yellow");
        assert_shields_endpoint_fields_only(&plus)?;
        assert_ne!(plus["message"], "UB-free");
        Ok(())
    }

    #[test]
    fn zero_gap_badge_json_still_names_open_gaps_not_safety() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let (main, plus) = render(&output);
        let main = parse_json(&main)?;
        let plus = parse_json(&plus)?;

        assert_eq!(main["message"], "0");
        assert_eq!(main["schemaVersion"], 1);
        assert_eq!(main["color"], "green");
        assert_shields_endpoint_fields_only(&main)?;
        assert_ne!(main["message"], "safe");
        assert_eq!(plus["message"], "0");
        assert_eq!(plus["schemaVersion"], 1);
        assert_shields_endpoint_fields_only(&plus)?;
        assert_ne!(plus["message"], "Miri-clean");
        Ok(())
    }

    #[test]
    fn badge_endpoint_messages_are_numeric_and_overclaim_free() -> Result<(), String> {
        for fixture in ["raw_pointer_alignment", "safe_code_no_cards"] {
            let output = fixture_output(fixture)?;
            let (main, plus) = render(&output);

            assert_badge_endpoint_contract("unsafe-review", "unsafe_review", &main)?;
            assert_badge_endpoint_contract("unsafe-review+", "unsafe_review_plus", &plus)?;
        }

        Ok(())
    }

    #[test]
    fn unsafe_review_plus_count_matches_evidence_quality_breakdown() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let (main, plus) = render(&output);
        let main = parse_json(&main)?;
        let plus = parse_json(&plus)?;

        assert_eq!(main["message"], "1");
        assert_eq!(plus["message"], "1");

        Ok(())
    }

    #[test]
    fn unsafe_review_plus_does_not_double_count_open_actionable_gaps() -> Result<(), String> {
        let output = AnalyzeOutput {
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            root: PathBuf::from("."),
            scope: Scope::Repo,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            summary: Summary {
                open_actionable_gaps: 7,
                contract_missing: 2,
                guard_missing: 3,
                guarded_unwitnessed: 5,
                ..Summary::default()
            },
            cards: Vec::new(),
            diff_scoped_files: BTreeSet::new(),
            coverage_snapshot: BTreeMap::new(),
        };
        let (main, plus) = render(&output);
        let main = parse_json(&main)?;
        let plus = parse_json(&plus)?;

        assert_eq!(main["message"], "7");
        assert_eq!(plus["message"], "10");
        assert_shields_endpoint_fields_only(&plus)?;

        Ok(())
    }

    #[test]
    fn public_badge_payloads_are_shields_endpoint_json() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let (main, plus) = render(&output);

        for text in [main, plus] {
            let badge = parse_json(&text)?;
            assert_shields_endpoint_fields_only(&badge)?;
            for internal in [
                "contract_version",
                "kind",
                "scope",
                "basis",
                "status",
                "counts",
            ] {
                assert!(
                    badge.get(internal).is_none(),
                    "public badge JSON must not contain internal field `{internal}`"
                );
            }
        }

        Ok(())
    }

    /// SPEC-0031: when a baseline is recorded, the main badge reflects movement-relevant
    /// gaps (`new_gaps + worsened_gaps`), not the full inherited debt.
    #[test]
    fn baseline_aware_badge_uses_new_and_worsened_gaps_not_full_debt() -> Result<(), String> {
        let output = AnalyzeOutput {
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            root: PathBuf::from("."),
            scope: Scope::Repo,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            summary: Summary {
                // 3 total open-actionable non-baseline cards (new), 5 baseline-known (inherited)
                open_actionable_gaps: 3,
                new_gaps: 3,
                worsened_gaps: 0,
                inherited_gaps: 5,
                resolved_gaps: 0,
                contract_missing: 2,
                guard_missing: 1,
                guarded_unwitnessed: 0,
                ..Summary::default()
            },
            cards: Vec::new(),
            diff_scoped_files: BTreeSet::new(),
            coverage_snapshot: BTreeMap::new(),
        };
        let (main, plus) = render(&output);
        let main = parse_json(&main)?;
        let plus = parse_json(&plus)?;

        // Main badge shows movement-relevant count (new_gaps + worsened_gaps = 3 + 0 = 3)
        assert_eq!(
            main["message"], "3",
            "baseline-present badge must show new_gaps, not full open_actionable_gaps"
        );
        // Plus badge shows evidence quality (contract_missing + guard_missing + guarded_unwitnessed)
        // which already excludes BaselineKnown cards
        assert_eq!(
            plus["message"], "3",
            "plus badge shows missing+weak evidence count (baseline-known excluded by classification)"
        );

        assert_shields_endpoint_fields_only(&main)?;
        assert_shields_endpoint_fields_only(&plus)?;
        assert_badge_endpoint_contract(
            "unsafe-review",
            "unsafe_review",
            &serde_json::to_string(&main).map_err(|e| e.to_string())?,
        )?;
        assert_badge_endpoint_contract(
            "unsafe-review+",
            "unsafe_review_plus",
            &serde_json::to_string(&plus).map_err(|e| e.to_string())?,
        )?;
        Ok(())
    }

    /// SPEC-0031: when worsened_gaps > 0 (coverage regression), those count in the baseline-aware
    /// badge for both the main badge and the plus badge.
    ///
    /// Worsened baseline cards are `BaselineKnown` class so they do not appear in the
    /// `contract_missing` / `guard_missing` / `guarded_unwitnessed` slot-bucket counters.
    /// The plus badge must add `worsened_gaps` explicitly to be movement-compatible with the
    /// main badge (bug #1687: plus badge was not baseline-movement-aware before this fix).
    #[test]
    fn baseline_aware_badge_includes_worsened_gaps() -> Result<(), String> {
        let output = AnalyzeOutput {
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            root: PathBuf::from("."),
            scope: Scope::Repo,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            summary: Summary {
                open_actionable_gaps: 2,
                new_gaps: 2,
                worsened_gaps: 1,
                inherited_gaps: 3,
                resolved_gaps: 0,
                contract_missing: 1,
                guard_missing: 1,
                guarded_unwitnessed: 0,
                ..Summary::default()
            },
            cards: Vec::new(),
            diff_scoped_files: BTreeSet::new(),
            coverage_snapshot: BTreeMap::new(),
        };
        let (main, plus) = render(&output);
        let main = parse_json(&main)?;
        let plus = parse_json(&plus)?;

        // Main badge = new_gaps + worsened_gaps = 2 + 1 = 3
        assert_eq!(
            main["message"], "3",
            "baseline-present badge must include worsened_gaps in the count"
        );
        // Plus badge = (contract_missing + guard_missing + guarded_unwitnessed) + worsened_gaps
        //            = (1 + 1 + 0) + 1 = 3
        // Worsened baseline cards are BaselineKnown and do not appear in the slot buckets;
        // adding worsened_gaps makes the plus badge movement-compatible with the main badge.
        assert_eq!(
            plus["message"], "3",
            "plus badge must include worsened_gaps to be movement-compatible with main badge (bug #1687)"
        );
        assert_shields_endpoint_fields_only(&main)?;
        assert_shields_endpoint_fields_only(&plus)?;
        Ok(())
    }

    /// SPEC-0031 bug #1687: plus badge brownfield scenario — worsened baseline card must not
    /// be silently dropped from the plus badge count.
    ///
    /// A brownfield repo with a baseline floor that has inherited debt plus a worsened card
    /// should show the worsened card in BOTH badges consistently.  Before the fix the plus
    /// badge counted only slot-bucket classes and silently dropped worsened BaselineKnown cards.
    #[test]
    fn plus_badge_brownfield_worsened_not_silently_dropped() -> Result<(), String> {
        let output = AnalyzeOutput {
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            root: PathBuf::from("."),
            scope: Scope::Repo,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            summary: Summary {
                // 0 new open-actionable cards; 1 inherited baseline card that worsened
                open_actionable_gaps: 0,
                new_gaps: 0,
                worsened_gaps: 1,
                inherited_gaps: 4,
                resolved_gaps: 0,
                // Slot-bucket counters are 0 because the only actionable concern is a worsened
                // BaselineKnown card (class BaselineKnown does not increment these counters).
                contract_missing: 0,
                guard_missing: 0,
                guarded_unwitnessed: 0,
                ..Summary::default()
            },
            cards: Vec::new(),
            diff_scoped_files: BTreeSet::new(),
            coverage_snapshot: BTreeMap::new(),
        };
        let (main, plus) = render(&output);
        let main = parse_json(&main)?;
        let plus = parse_json(&plus)?;

        // Main badge: baseline present, so new_gaps + worsened_gaps = 0 + 1 = 1.
        assert_eq!(
            main["message"], "1",
            "main badge must count worsened_gaps when baseline is present"
        );
        // Plus badge MUST also show 1, not 0.
        // Before the fix this would return "0" (silent drop) because worsened BaselineKnown
        // cards were not included in the slot-bucket counters.
        assert_eq!(
            plus["message"], "1",
            "plus badge must not silently drop worsened baseline cards (bug #1687)"
        );
        assert_shields_endpoint_fields_only(&main)?;
        assert_shields_endpoint_fields_only(&plus)?;
        assert_badge_endpoint_contract(
            "unsafe-review",
            "unsafe_review",
            &serde_json::to_string(&main).map_err(|e| e.to_string())?,
        )?;
        assert_badge_endpoint_contract(
            "unsafe-review+",
            "unsafe_review_plus",
            &serde_json::to_string(&plus).map_err(|e| e.to_string())?,
        )?;
        Ok(())
    }

    /// SPEC-0031: when resolved_gaps > 0 (some baseline entries resolved) but no inherited gaps,
    /// a baseline is still considered present and the badge uses movement counts.
    #[test]
    fn resolved_gaps_signal_baseline_is_present() -> Result<(), String> {
        let output = AnalyzeOutput {
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            root: PathBuf::from("."),
            scope: Scope::Repo,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            summary: Summary {
                open_actionable_gaps: 1,
                new_gaps: 1,
                worsened_gaps: 0,
                inherited_gaps: 0,
                resolved_gaps: 2, // baseline entries resolved — baseline is present
                contract_missing: 1,
                guard_missing: 0,
                guarded_unwitnessed: 0,
                ..Summary::default()
            },
            cards: Vec::new(),
            diff_scoped_files: BTreeSet::new(),
            coverage_snapshot: BTreeMap::new(),
        };
        let (main, _plus) = render(&output);
        let main = parse_json(&main)?;

        // Baseline present (resolved_gaps > 0), so badge uses new_gaps = 1
        assert_eq!(
            main["message"], "1",
            "resolved_gaps signals baseline presence; badge uses new_gaps"
        );
        assert_shields_endpoint_fields_only(&main)?;
        Ok(())
    }

    /// SPEC-0031: no-baseline fallback — when no baseline floor is recorded, the badge
    /// shows raw open_actionable_gaps (the honest "no floor set yet" reading).
    #[test]
    fn no_baseline_fallback_uses_raw_open_actionable_gaps() -> Result<(), String> {
        let output = AnalyzeOutput {
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            root: PathBuf::from("."),
            scope: Scope::Repo,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            summary: Summary {
                // No baseline: new_gaps == open_actionable_gaps, inherited_gaps == 0
                open_actionable_gaps: 5,
                new_gaps: 5,
                worsened_gaps: 0,
                inherited_gaps: 0,
                resolved_gaps: 0,
                contract_missing: 3,
                guard_missing: 2,
                guarded_unwitnessed: 0,
                ..Summary::default()
            },
            cards: Vec::new(),
            diff_scoped_files: BTreeSet::new(),
            coverage_snapshot: BTreeMap::new(),
        };
        let (main, plus) = render(&output);
        let main = parse_json(&main)?;
        let plus = parse_json(&plus)?;

        // No baseline: falls back to open_actionable_gaps = 5
        assert_eq!(
            main["message"], "5",
            "no-baseline fallback must use raw open_actionable_gaps"
        );
        assert_eq!(
            plus["message"], "5",
            "plus badge shows raw evidence quality count"
        );
        assert_shields_endpoint_fields_only(&main)?;
        assert_shields_endpoint_fields_only(&plus)?;
        Ok(())
    }

    /// SPEC-0031: overclaim-term rejection covers baseline-aware outputs too.
    #[test]
    fn baseline_aware_badge_payloads_are_overclaim_free() -> Result<(), String> {
        let output = AnalyzeOutput {
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            root: PathBuf::from("."),
            scope: Scope::Repo,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            summary: Summary {
                open_actionable_gaps: 1,
                new_gaps: 1,
                inherited_gaps: 3,
                contract_missing: 1,
                ..Summary::default()
            },
            cards: Vec::new(),
            diff_scoped_files: BTreeSet::new(),
            coverage_snapshot: BTreeMap::new(),
        };
        let (main, plus) = render(&output);
        assert_badge_endpoint_contract("unsafe-review", "unsafe_review", &main)?;
        assert_badge_endpoint_contract("unsafe-review+", "unsafe_review_plus", &plus)?;
        Ok(())
    }

    fn assert_badge_endpoint_contract(
        expected_label: &str,
        _expected_kind: &str,
        text: &str,
    ) -> Result<(), String> {
        let badge = parse_json(text)?;
        assert_eq!(badge["schemaVersion"], 1);
        assert_eq!(badge["label"], expected_label);
        assert_shields_endpoint_fields_only(&badge)?;

        let message = badge["message"]
            .as_str()
            .ok_or_else(|| "badge message must be a string".to_string())?;
        assert!(
            !message.is_empty() && message.chars().all(|ch| ch.is_ascii_digit()),
            "badge message must be a numeric count, got {message:?}"
        );

        let lowercase = text.to_ascii_lowercase();
        for forbidden in [
            "all clear",
            "ub-free",
            "miri-clean",
            "verified",
            "proof",
            "policy-ready",
            "blocking-ready",
            "site execution",
            "memory-safety",
        ] {
            assert!(
                !lowercase.contains(forbidden),
                "badge endpoint JSON must not contain overclaim term {forbidden:?}: {text}"
            );
        }

        Ok(())
    }

    fn assert_shields_endpoint_fields_only(badge: &serde_json::Value) -> Result<(), String> {
        let object = badge
            .as_object()
            .ok_or_else(|| "badge endpoint JSON must be an object".to_string())?;
        for key in object.keys() {
            if !["schemaVersion", "label", "message", "color"].contains(&key.as_str()) {
                return Err(format!(
                    "public badge JSON contains non-Shields field `{key}`"
                ));
            }
        }
        Ok(())
    }

    fn fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name);
        crate::api::analyze(AnalyzeInput {
            root,
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })
    }

    fn parse_json(text: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str(text).map_err(|err| format!("JSON parse failed: {err}"))
    }
}
