//! Calibration snapshot synchronization and count rewriting.

use crate::{
    ACCURACY_CALIBRATION_REPORT, AccuracyCalibrationReportStats, OBJECTIVE_AUDIT,
    accuracy_calibration_report_stats, accuracy_labels, calibration_manifest, parse_toml_file,
    read_to_string, workspace_path,
};
use std::fs;

fn calibration_stats_for_sync() -> Result<AccuracyCalibrationReportStats, String> {
    let manifest = calibration_manifest::validate()?;
    let accuracy_policy = parse_toml_file(&workspace_path("policy/accuracy-calibration.toml"))?;
    let label_count =
        accuracy_labels::check_accuracy_label_ledgers(&accuracy_policy, &manifest.fixture_cases)?;
    accuracy_calibration_report_stats(&accuracy_policy, manifest.case_count, label_count)
}

pub(crate) fn sync_calibration_snapshot() -> Result<(), String> {
    let stats = calibration_stats_for_sync()?;
    sync_calibration_report(&stats)?;
    sync_objective_audit_snapshot(&stats)?;
    println!("sync-calibration-snapshot: ok");
    Ok(())
}

fn sync_calibration_report(stats: &AccuracyCalibrationReportStats) -> Result<(), String> {
    let path = workspace_path(ACCURACY_CALIBRATION_REPORT);
    let original = read_to_string(&path)?;
    let updated = rewrite_calibration_report_counts(&original, stats)?;
    if original == updated {
        println!("{ACCURACY_CALIBRATION_REPORT}: counts already current, no change");
    } else {
        fs::write(&path, &updated)
            .map_err(|err| format!("write {ACCURACY_CALIBRATION_REPORT} failed: {err}"))?;
        println!("{ACCURACY_CALIBRATION_REPORT}: updated count lines");
    }
    Ok(())
}

fn rewrite_calibration_report_counts(
    text: &str,
    stats: &AccuracyCalibrationReportStats,
) -> Result<String, String> {
    let mut result = text.to_string();

    // Update all nine count lines in the ## Counts block.
    for (prefix, value) in [
        ("- Claims: ", stats.claim_count),
        ("- Fixture-pinned claims: ", stats.fixture_pinned_claims),
        ("- Dogfood-measured claims: ", stats.dogfood_measured_claims),
        (
            "- Labeled-calibrated claims: ",
            stats.labeled_calibrated_claims,
        ),
        ("- Policy-eligible claims: ", stats.policy_eligible_claims),
        ("- Calibration cases: ", stats.calibration_case_count),
        ("- Label ledgers: ", stats.label_ledger_count),
        ("- Label samples: ", stats.label_sample_count),
        ("- Labeled reports: ", stats.labeled_report_count),
    ] {
        result = replace_prefixed_count_line(
            &result,
            prefix,
            &value.to_string(),
            ACCURACY_CALIBRATION_REPORT,
        )?;
    }

    Ok(result)
}

fn replace_prefixed_count_line(
    text: &str,
    prefix: &str,
    new_value: &str,
    path: &str,
) -> Result<String, String> {
    let start = text
        .find(prefix)
        .ok_or_else(|| format!("{path} is missing expected line prefix `{prefix}`"))?;
    let after_prefix = start + prefix.len();
    // Find the end of the value (rest of the line up to newline or end of string).
    let end = text[after_prefix..]
        .find('\n')
        .map_or(text.len(), |rel| after_prefix + rel);
    let mut result = text.to_string();
    result.replace_range(after_prefix..end, new_value);
    Ok(result)
}

fn sync_objective_audit_snapshot(stats: &AccuracyCalibrationReportStats) -> Result<(), String> {
    let path = workspace_path(OBJECTIVE_AUDIT);
    let original = read_to_string(&path)?;
    let updated = rewrite_objective_audit_counts(&original, stats)?;
    if original == updated {
        println!("{OBJECTIVE_AUDIT}: counts already current, no change");
    } else {
        fs::write(&path, &updated)
            .map_err(|err| format!("write {OBJECTIVE_AUDIT} failed: {err}"))?;
        println!("{OBJECTIVE_AUDIT}: updated calibration snapshot counts");
    }
    Ok(())
}

fn rewrite_objective_audit_counts(
    text: &str,
    stats: &AccuracyCalibrationReportStats,
) -> Result<String, String> {
    // The checked sentence uses four `N <phrase>` patterns where N and the phrase
    // may be separated by a line break due to wrapping. Each entry lists the
    // search anchors in preference order (exact match first, then line-wrapped
    // variant). We walk back over whitespace from the anchor to locate the digit
    // run and replace it in place.
    let anchors: &[(&[&str], usize)] = &[
        (&["fixture-pinned claims"], stats.fixture_pinned_claims),
        (&["calibration cases"], stats.calibration_case_count),
        (&["label ledgers"], stats.label_ledger_count),
        // "label samples" may be line-wrapped as "label\nsamples".
        (
            &["label samples", "label\nsamples"],
            stats.label_sample_count,
        ),
    ];
    let mut result = text.to_string();
    for (candidates, new_value) in anchors {
        result = replace_number_before_anchor(&result, candidates, *new_value, OBJECTIVE_AUDIT)?;
    }
    Ok(result)
}

fn replace_number_before_anchor(
    text: &str,
    anchors: &[&str],
    new_value: usize,
    path: &str,
) -> Result<String, String> {
    // Find the first occurrence of any of the anchor strings.
    let anchor_pos = anchors
        .iter()
        .filter_map(|anchor| text.find(anchor))
        .min()
        .ok_or_else(|| {
            format!(
                "{path} is missing expected phrase `<N> {}`; cannot update count",
                anchors[0]
            )
        })?;

    // `before` is the text up to the anchor. Trim trailing whitespace to reach
    // the end of the digit run, then trim digits to find the digit start.
    let before = &text[..anchor_pos];
    let trimmed = before.trim_end_matches(|c: char| c.is_ascii_whitespace());
    let digit_end = trimmed.len();
    let digit_start = trimmed
        .rfind(|c: char| !c.is_ascii_digit())
        .map_or(0, |pos| pos + 1);
    if digit_start == digit_end {
        return Err(format!(
            "{path} has no digit before `{}`; expected `<N> {}`",
            anchors[0], anchors[0]
        ));
    }
    let mut result = text.to_string();
    result.replace_range(digit_start..digit_end, &new_value.to_string());
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{rewrite_calibration_report_counts, rewrite_objective_audit_counts};
    use crate::tests::{test_accuracy_report_stats, test_accuracy_report_text};

    #[test]
    fn rewrite_calibration_report_counts_updates_all_count_lines() -> Result<(), String> {
        let stats = test_accuracy_report_stats();
        let original = test_accuracy_report_text();
        let stale = original
            .replace("- Claims: 1", "- Claims: 99")
            .replace("- Fixture-pinned claims: 1", "- Fixture-pinned claims: 99")
            .replace(
                "- Dogfood-measured claims: 0",
                "- Dogfood-measured claims: 99",
            )
            .replace(
                "- Labeled-calibrated claims: 0",
                "- Labeled-calibrated claims: 99",
            )
            .replace(
                "- Policy-eligible claims: 0",
                "- Policy-eligible claims: 99",
            )
            .replace("- Calibration cases: 2", "- Calibration cases: 99")
            .replace("- Label ledgers: 1", "- Label ledgers: 99")
            .replace("- Label samples: 2", "- Label samples: 99")
            .replace("- Labeled reports: 0", "- Labeled reports: 99");

        let result = rewrite_calibration_report_counts(&stale, &stats)?;

        assert!(result.contains("- Claims: 1"));
        assert!(result.contains("- Fixture-pinned claims: 1"));
        assert!(result.contains("- Dogfood-measured claims: 0"));
        assert!(result.contains("- Labeled-calibrated claims: 0"));
        assert!(result.contains("- Policy-eligible claims: 0"));
        assert!(result.contains("- Calibration cases: 2"));
        assert!(result.contains("- Label ledgers: 1"));
        assert!(result.contains("- Label samples: 2"));
        assert!(result.contains("- Labeled reports: 0"));
        assert!(result.contains("No global precision/recall claim"));
        assert!(result.contains("No policy readiness claim"));
        Ok(())
    }

    #[test]
    fn rewrite_calibration_report_counts_no_change_when_current() -> Result<(), String> {
        let stats = test_accuracy_report_stats();
        let original = test_accuracy_report_text();
        let result = rewrite_calibration_report_counts(&original, &stats)?;
        assert_eq!(result, original);
        Ok(())
    }

    #[test]
    fn rewrite_objective_audit_counts_updates_four_tokens() -> Result<(), String> {
        let stats = test_accuracy_report_stats();
        let text = "The checked report currently records 99 fixture-pinned claims, 99 calibration cases, 99 label ledgers, and 99 label samples.";
        let result = rewrite_objective_audit_counts(text, &stats)?;
        assert!(result.contains("1 fixture-pinned claims"));
        assert!(result.contains("2 calibration cases"));
        assert!(result.contains("1 label ledgers"));
        assert!(result.contains("2 label samples"));
        Ok(())
    }

    #[test]
    fn rewrite_objective_audit_counts_handles_line_wrap() -> Result<(), String> {
        let stats = test_accuracy_report_stats();
        let text = "records 99\nfixture-pinned claims, 99 calibration cases, 99 label ledgers, and 99 label\nsamples.";
        let result = rewrite_objective_audit_counts(text, &stats)?;
        assert!(result.contains("1\nfixture-pinned claims"));
        assert!(result.contains("2 calibration cases"));
        assert!(result.contains("1 label ledgers"));
        assert!(result.contains("2 label\nsamples"));
        Ok(())
    }

    #[test]
    fn rewrite_objective_audit_counts_no_change_when_current() -> Result<(), String> {
        let stats = test_accuracy_report_stats();
        let text = "The checked report currently records 1 fixture-pinned claims, 2 calibration cases, 1 label ledgers, and 2 label samples.";
        let result = rewrite_objective_audit_counts(text, &stats)?;
        assert_eq!(result, text);
        Ok(())
    }

    #[test]
    fn rewrite_objective_audit_counts_preserves_surrounding_prose() -> Result<(), String> {
        let stats = test_accuracy_report_stats();
        let text = "Some leading prose. The checked report currently records 1 fixture-pinned claims, 2 calibration cases, 1 label ledgers, and 2 label samples. Some trailing prose.";
        let result = rewrite_objective_audit_counts(text, &stats)?;
        assert!(result.starts_with("Some leading prose."));
        assert!(result.ends_with("Some trailing prose."));
        Ok(())
    }
}
