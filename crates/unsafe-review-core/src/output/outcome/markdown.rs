use super::{OutcomeCard, OutcomeCardState, OutcomeReport};
use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};

const MAX_MOVEMENT_REASONS: usize = 5;

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
        out.push_str("| Card | Class | Priority | Operation family | Missing | Next action |\n");
        out.push_str("|---|---|---|---|---:|---|\n");
        for gap in &report.reviewer_delta.top_remaining_gaps {
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | `{}` | {} | {} |\n",
                gap.card_id,
                gap.class_name,
                gap.priority,
                markdown_cell(gap.operation_family.as_deref().unwrap_or("unknown")),
                gap.missing_count,
                markdown_cell(gap.next_action.as_deref().unwrap_or(""))
            ));
        }
        out.push('\n');
    }
    out.push_str("## Movement reasons\n\n");
    let movement_reasons = movement_reasons(report);
    if movement_reasons.is_empty() {
        out.push_str(
            "- No new, resolved, improved, or regressed ReviewCards in these saved snapshots.\n\n",
        );
    } else {
        for (status, card) in &movement_reasons {
            out.push_str(&format!(
                "- `{status}` `{}`: {}\n",
                card.card_id,
                markdown_cell(&card.reason)
            ));
        }
        let remaining = movement_count(report).saturating_sub(movement_reasons.len());
        if remaining > 0 {
            out.push_str(&format!(
                "- Additional movement reasons: {remaining} more in the Card outcomes table.\n"
            ));
        }
        out.push_str("\n");
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

fn movement_reasons(report: &OutcomeReport) -> Vec<(&'static str, &OutcomeCard)> {
    let mut reasons = Vec::new();
    for (status, cards) in [
        ("new", report.cards.new.as_slice()),
        ("regressed", report.cards.regressed.as_slice()),
        ("improved", report.cards.improved.as_slice()),
        ("resolved", report.cards.resolved.as_slice()),
    ] {
        for card in cards {
            if reasons.len() == MAX_MOVEMENT_REASONS {
                return reasons;
            }
            reasons.push((status, card));
        }
    }
    reasons
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
            if let Some(operation_family) = state.operation_family.as_deref() {
                parts.push(format!(
                    "operation family `{}`",
                    markdown_cell(operation_family)
                ));
            }
            if let Some(operation) = state.operation.as_deref() {
                parts.push(format!("operation `{}`", markdown_cell(operation)));
            }
            if let Some(next_action) = state.next_action.as_deref() {
                parts.push(format!("next: {}", markdown_cell(next_action)));
            }
            parts.join("; ")
        }
        None => "-".to_string(),
    }
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}
