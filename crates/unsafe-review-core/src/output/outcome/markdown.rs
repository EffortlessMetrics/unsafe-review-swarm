use super::{OutcomeCardState, OutcomeReport};
use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};

pub(super) fn render_markdown(report: &OutcomeReport) -> String {
    let mut out = String::new();
    out.push_str("# unsafe-review outcome\n\n");
    out.push_str("Static comparison of two existing unsafe-review JSON snapshots.\n\n");
    out.push_str("## Summary\n\n");
    out.push_str("| New | Resolved | Improved | Regressed | Unchanged |\n");
    out.push_str("|---:|---:|---:|---:|---:|\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} |\n\n",
        report.summary.new,
        report.summary.resolved,
        report.summary.improved,
        report.summary.regressed,
        report.summary.unchanged
    ));
    out.push_str("## Reviewer delta\n\n");
    out.push_str(&format!(
        "- New cards: {}\n",
        report.reviewer_delta.new_cards
    ));
    out.push_str(&format!(
        "- Resolved cards: {}\n",
        report.reviewer_delta.resolved_cards
    ));
    out.push_str(&format!(
        "- Improved cards: {}\n",
        report.reviewer_delta.improved_cards
    ));
    out.push_str(&format!(
        "- Regressed cards: {}\n",
        report.reviewer_delta.regressed_cards
    ));
    out.push_str(&format!(
        "- Receipt movement: {} improved, {} regressed\n",
        report.reviewer_delta.receipt_movement.improved,
        report.reviewer_delta.receipt_movement.regressed
    ));
    if report.reviewer_delta.top_remaining_gaps.is_empty() {
        out.push_str("- Top remaining gaps: none in the after snapshot\n\n");
    } else {
        out.push_str("\nTop remaining gaps:\n\n");
        out.push_str(
            "| Card | Class | Priority | Proof path | Operation family | Missing | Next action |\n",
        );
        out.push_str("|---|---|---|---|---|---:|---|\n");
        for gap in &report.reviewer_delta.top_remaining_gaps {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | `{}` | `{}` | {} | {} |\n",
                gap.card_id,
                gap.class_name,
                gap.priority,
                markdown_cell(gap.proof_path.as_deref().unwrap_or("unknown")),
                markdown_cell(gap.operation_family.as_deref().unwrap_or("unknown")),
                gap.missing_count,
                markdown_cell(gap.next_action.as_deref().unwrap_or(""))
            ));
        }
        out.push('\n');
    }
    out.push_str("## Movement reasons\n\n");
    if report.reviewer_delta.movement_reasons.is_empty() {
        out.push_str(
            "- No new, resolved, improved, or regressed ReviewCards in these saved snapshots.\n\n",
        );
    } else {
        for reason in &report.reviewer_delta.movement_reasons {
            out.push_str(&format!(
                "- `{status}` `{}`: {}\n",
                reason.card_id,
                markdown_cell(&reason.reason),
                status = reason.status
            ));
        }
        let remaining =
            movement_count(report).saturating_sub(report.reviewer_delta.movement_reasons.len());
        if remaining > 0 {
            out.push_str(&format!(
                "- Additional movement reasons: {remaining} more in the Card outcomes table.\n"
            ));
        }
        out.push('\n');
    }
    out.push_str("## Card outcomes\n\n");
    if report.cards.is_empty() {
        out.push_str(NO_CHANGED_GAPS_MESSAGE);
        out.push_str(" No ReviewCards were present in either saved snapshot.\n");
        out.push_str(NO_CHANGED_GAPS_LIMITATION);
        out.push_str("\n\n");
    } else {
        out.push_str("| Status | Card | Reason | Before | After |\n");
        out.push_str("|---|---|---|---|---|\n");
        for (status, cards) in report.cards.groups() {
            for card in cards {
                out.push_str(&format!(
                    "| `{status}` | `{}` | {} | {} | {} |\n",
                    card.card_id,
                    card.reason,
                    markdown_state(card.before.as_ref()),
                    markdown_state(card.after.as_ref())
                ));
            }
        }
        out.push('\n');
    }
    out.push_str("## Limitations\n\n");
    for limitation in &report.limitations {
        out.push_str("- ");
        out.push_str(limitation);
        out.push('\n');
    }
    out.push('\n');
    out.push_str("## Trust boundary\n\n");
    out.push_str(&report.trust_boundary);
    out.push('\n');
    out
}

fn movement_count(report: &OutcomeReport) -> usize {
    report.summary.new
        + report.summary.resolved
        + report.summary.improved
        + report.summary.regressed
}

fn markdown_state(state: Option<&OutcomeCardState>) -> String {
    match state {
        Some(state) => {
            let mut parts = vec![format!(
                "`{}` / `{}` / {} missing / witness `{}`",
                state.class_name, state.priority, state.missing_count, state.witness
            )];
            if let Some(source) = state.source.as_deref() {
                parts.push(format!("source `{}`", markdown_cell(source)));
            }
            if let Some(manual_candidate) = state.manual_candidate {
                parts.push(format!("manual_candidate `{manual_candidate}`"));
            }
            if let Some(analyzer_discovered) = state.analyzer_discovered {
                parts.push(format!("analyzer-discovered `{analyzer_discovered}`"));
            }
            if let Some(title) = state.title.as_deref() {
                parts.push(format!("title `{}`", markdown_cell(title)));
            }
            if let Some(location) = state.location.as_ref() {
                parts.push(format!(
                    "location `{}`:{}",
                    markdown_cell(&location.file),
                    location.line
                ));
            }
            if let Some(operation_family) = state.operation_family.as_deref() {
                parts.push(format!(
                    "operation family `{}`",
                    markdown_cell(operation_family)
                ));
            }
            if let Some(proof_path) = state.proof_path.as_deref() {
                parts.push(format!("proof path `{}`", markdown_cell(proof_path)));
            }
            if let Some(operation) = state.operation.as_deref() {
                parts.push(format!("operation `{}`", markdown_cell(operation)));
            }
            if let Some(evidence_count) = state.evidence_count {
                parts.push(format!("external evidence {evidence_count}"));
            }
            if let Some(safe_caller) = state.safe_caller.as_deref() {
                parts.push(format!("route `{}`", markdown_cell(safe_caller)));
            }
            if let Some(invariant) = state.invariant.as_deref() {
                parts.push(format!("invariant {}", markdown_cell(invariant)));
            }
            if let Some(oracle_map) = state.oracle_map.as_ref() {
                parts.push(format!(
                    "oracle `{}` `{}` / `{}` / confidence `{}` / limitation {}",
                    markdown_cell(&oracle_map.oracle_language),
                    markdown_cell(&oracle_map.oracle_path.display().to_string()),
                    markdown_cell(&oracle_map.oracle_kind),
                    markdown_cell(&oracle_map.coverage_confidence),
                    markdown_cell(&oracle_map.limitation)
                ));
            }
            if let Some(evidence) = state.evidence.first() {
                let mut evidence_parts = vec![format!(
                    "first evidence `{}`",
                    markdown_cell(&evidence.kind)
                )];
                if let Some(path) = evidence.path.as_deref() {
                    evidence_parts.push(format!("path `{}`", markdown_cell(path)));
                }
                if let Some(command) = evidence.command.as_deref() {
                    evidence_parts.push(format!("command `{}`", markdown_cell(command)));
                }
                if let Some(limitation) = evidence.limitation.as_deref() {
                    evidence_parts.push(format!("limitation {}", markdown_cell(limitation)));
                }
                parts.push(evidence_parts.join(", "));
            }
            if let Some(first_fix) = state.fix_options.first() {
                parts.push(format!("first fix: {}", markdown_cell(first_fix)));
            }
            if let Some(first_test) = state.test_targets.first() {
                parts.push(format!("first test: {}", markdown_cell(first_test)));
            }
            if let Some(first_non_goal) = state.do_not_touch.first() {
                parts.push(format!("first non-goal: {}", markdown_cell(first_non_goal)));
            }
            if let Some(next_action) = state.next_action.as_deref() {
                parts.push(format!("next: {}", markdown_cell(next_action)));
            }
            if let Some(trust_boundary) = state.trust_boundary.as_deref() {
                parts.push(format!("boundary: {}", markdown_cell(trust_boundary)));
            }
            parts.join("; ")
        }
        None => "-".to_string(),
    }
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}
