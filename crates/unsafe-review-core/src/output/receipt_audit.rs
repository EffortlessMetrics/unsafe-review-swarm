use crate::analysis::receipts::{ReceiptAuditCard, ReceiptAuditReport};

pub(crate) fn render_json(report: &ReceiptAuditReport) -> String {
    match serde_json::to_string_pretty(report) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"receipt audit serialization failed: {err}\"\n}}"),
    }
}

pub(crate) fn render_markdown(report: &ReceiptAuditReport) -> String {
    let mut out = String::new();
    out.push_str("# unsafe-review receipt audit\n\n");
    out.push_str("Static audit of saved witness receipt metadata against current ReviewCards.\n\n");
    out.push_str(&format!("Audit date: `{}`\n\n", report.audit_date));
    out.push_str("## Summary\n\n");
    out.push_str("| Receipts | Matched | Unmatched | Expired | Stale | Wrong identity | Wrong tool | Weak strength | Command hash mismatch | Duplicate | Invalid |\n");
    out.push_str("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n\n",
        report.summary.receipts,
        report.summary.matched,
        report.summary.unmatched,
        report.summary.expired,
        report.summary.stale,
        report.summary.wrong_identity,
        report.summary.wrong_tool,
        report.summary.weaker_than_required,
        report.summary.command_hash_mismatch,
        report.summary.duplicate,
        report.summary.invalid
    ));
    out.push_str("## Receipts\n\n");
    if report.receipts.is_empty() {
        out.push_str("No receipt files found.\n\n");
    } else {
        out.push_str(
            "| Status | Receipt | Card | Matched card | Tool | Strength | Summary | Author | Recorded | Expires | Command hash | Limitations | Routed tools | Issues |\n",
        );
        out.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
        for receipt in &report.receipts {
            out.push_str(&receipt_row(receipt));
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

fn receipt_row(receipt: &crate::analysis::receipts::ReceiptAuditEntry) -> String {
    format!(
        "| {} | `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
        markdown_cell(&receipt.statuses.join(", ")),
        receipt.path,
        optional_code(receipt.card_id.as_deref()),
        matched_card(receipt.matched_card.as_ref()),
        optional_code(receipt.receipt_tool.as_deref()),
        optional_code(receipt.strength.as_deref()),
        summary_cell(receipt.summary.as_deref()),
        optional_code(receipt.author.as_deref()),
        optional_code(receipt.recorded_at.as_deref()),
        optional_code(receipt.expires_at.as_deref()),
        optional_code(receipt.command_hash.as_deref()),
        limitations_cell(&receipt.limitations),
        route_tools(&receipt.route_tools),
        issues_cell(&receipt.issues)
    )
}

fn summary_cell(summary: Option<&str>) -> String {
    match summary {
        Some(summary) if !summary.is_empty() => markdown_cell(summary),
        _ => "-".to_string(),
    }
}

fn limitations_cell(limitations: &[String]) -> String {
    if limitations.is_empty() {
        return "-".to_string();
    }
    markdown_cell(&limitations.join("; "))
}

fn route_tools(tools: &[String]) -> String {
    if tools.is_empty() {
        return "-".to_string();
    }
    tools
        .iter()
        .map(|tool| format!("`{}`", markdown_cell(tool)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn issues_cell(issues: &[String]) -> String {
    if issues.is_empty() {
        return "-".to_string();
    }
    markdown_cell(&issues.join("; "))
}

fn optional_code(value: Option<&str>) -> String {
    match value {
        Some(value) if !value.is_empty() => format!("`{value}`"),
        _ => "-".to_string(),
    }
}

fn matched_card(card: Option<&ReceiptAuditCard>) -> String {
    let Some(card) = card else {
        return "-".to_string();
    };
    format!(
        "`{}` / `{}` / `{}` / {} missing; next: {}",
        card.class_name,
        card.operation_family,
        markdown_cell(&card.operation),
        card.missing_count,
        markdown_cell(&card.next_action)
    )
}

fn markdown_cell(value: &str) -> String {
    value.replace('|', "\\|").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::receipts::{ReceiptAuditEntry, ReceiptAuditReport, ReceiptAuditSummary};

    #[test]
    fn markdown_projects_problem_receipt_statuses_and_limits() {
        let report = ReceiptAuditReport {
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            mode: "receipt-audit".to_string(),
            policy: "advisory".to_string(),
            audit_date: "2026-05-26".to_string(),
            trust_boundary: "Static witness receipt audit only; does not execute witnesses and does not prove site reach.".to_string(),
            limitations: vec![
                "audits saved witness receipt metadata only".to_string(),
                "does not execute Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux".to_string(),
                "matched receipts improve witness evidence only and do not erase missing contracts, guards, or reach evidence".to_string(),
            ],
            summary: ReceiptAuditSummary {
                receipts: 2,
                matched: 1,
                unmatched: 1,
                expired: 0,
                stale: 1,
                wrong_identity: 0,
                wrong_tool: 1,
                weaker_than_required: 1,
                command_hash_mismatch: 1,
                duplicate: 0,
                invalid: 0,
            },
            receipts: vec![
                ReceiptAuditEntry {
                    path: ".unsafe-review/receipts/stale.json".to_string(),
                    card_id: Some(
                        "UR-stale-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                            .to_string(),
                    ),
                    receipt_tool: Some("miri".to_string()),
                    strength: Some("ran".to_string()),
                    summary: None,
                    author: Some("core/fixtures".to_string()),
                    recorded_at: Some("2026-05-20T00:00:00Z".to_string()),
                    expires_at: Some("2026-08-18".to_string()),
                    command_hash: None,
                    limitations: Vec::new(),
                    statuses: vec!["stale".to_string(), "unmatched".to_string()],
                    issues: vec![
                        "receipt card_id is not present in the current ReviewCard set".to_string(),
                    ],
                    matched_card: None,
                    route_tools: Vec::new(),
                },
                ReceiptAuditEntry {
                    path: ".unsafe-review/receipts/weak-wrong-tool.json".to_string(),
                    card_id: Some(
                        "UR-live-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                            .to_string(),
                    ),
                    receipt_tool: Some("loom".to_string()),
                    strength: Some("configured".to_string()),
                    summary: Some("focused witness".to_string()),
                    author: Some("core/fixtures".to_string()),
                    recorded_at: Some("2026-05-20T00:00:00Z".to_string()),
                    expires_at: Some("2026-08-18".to_string()),
                    command_hash: Some("4ce9d7c8eeb19a30".to_string()),
                    limitations: vec!["fixture only".to_string()],
                    statuses: vec![
                        "command_hash_mismatch".to_string(),
                        "matched".to_string(),
                        "weaker_than_required".to_string(),
                        "wrong_tool".to_string(),
                    ],
                    issues: vec![
                        "receipt tool `loom` is not one of this card's routed witness tools: miri, cargo-careful".to_string(),
                        "receipt strength `configured` is weaker than the minimum `ran` strength for a required witness route".to_string(),
                    ],
                    matched_card: Some(ReceiptAuditCard {
                        id: "UR-live-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                            .to_string(),
                        class_name: "guard_missing",
                        operation: "unsafe { ptr.cast::<Header>().read() }".to_string(),
                        operation_family: "raw_pointer_read",
                        missing_count: 2,
                        next_action: "Add or expose guard | witness\nThen attach receipt"
                            .to_string(),
                    }),
                    route_tools: vec!["miri".to_string(), "cargo-careful".to_string()],
                },
            ],
        };

        let markdown = render_markdown(&report);

        assert!(markdown.contains("# unsafe-review receipt audit"));
        assert!(markdown.contains("Audit date: `2026-05-26`"));
        assert!(markdown.contains("Command hash mismatch"));
        assert!(markdown.contains("| 2 | 1 | 1 | 0 | 1 | 0 | 1 | 1 | 1 | 0 | 0 |"));
        assert!(markdown.contains(
            "| Status | Receipt | Card | Matched card | Tool | Strength | Summary | Author | Recorded | Expires | Command hash | Limitations | Routed tools | Issues |"
        ));
        assert!(markdown.contains("focused witness"));
        assert!(markdown.contains("`core/fixtures`"));
        assert!(markdown.contains("`2026-05-20T00:00:00Z`"));
        assert!(markdown.contains("`2026-08-18`"));
        assert!(markdown.contains("`4ce9d7c8eeb19a30`"));
        assert!(markdown.contains("fixture only"));
        assert!(markdown.contains("`miri`, `cargo-careful`"));
        assert!(markdown.contains("stale, unmatched"));
        assert!(
            markdown.contains("command_hash_mismatch, matched, weaker_than_required, wrong_tool")
        );
        assert!(markdown.contains("receipt card_id is not present in the current ReviewCard set"));
        assert!(
            markdown.contains("receipt tool `loom` is not one of this card's routed witness tools")
        );
        assert!(markdown.contains("receipt strength `configured` is weaker"));
        assert!(markdown.contains("`guard_missing` / `raw_pointer_read`"));
        assert!(markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
        assert!(markdown.contains("Add or expose guard \\| witness Then attach receipt"));
        assert!(markdown.contains("## Limitations"));
        assert!(markdown.contains("does not execute Miri"));
        assert!(markdown.contains("do not erase missing contracts"));
        assert!(markdown.contains("## Trust boundary"));
        assert!(markdown.contains("does not execute witnesses"));
        assert!(markdown.contains("does not prove site reach"));
    }
}
