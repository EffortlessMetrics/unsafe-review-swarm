use crate::analysis::receipts::ReceiptAuditReport;

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
    out.push_str("## Summary\n\n");
    out.push_str("| Receipts | Matched | Unmatched | Expired | Stale | Wrong identity | Wrong tool | Weak strength | Duplicate | Invalid |\n");
    out.push_str("|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n\n",
        report.summary.receipts,
        report.summary.matched,
        report.summary.unmatched,
        report.summary.expired,
        report.summary.stale,
        report.summary.wrong_identity,
        report.summary.wrong_tool,
        report.summary.weaker_than_required,
        report.summary.duplicate,
        report.summary.invalid
    ));
    out.push_str("## Receipts\n\n");
    if report.receipts.is_empty() {
        out.push_str("No receipt files found.\n\n");
    } else {
        out.push_str("| Status | Receipt | Card | Tool | Strength | Issues |\n");
        out.push_str("|---|---|---|---|---|---|\n");
        for receipt in &report.receipts {
            out.push_str(&format!(
                "| {} | `{}` | {} | {} | {} | {} |\n",
                markdown_cell(&receipt.statuses.join(", ")),
                markdown_cell(&receipt.path),
                optional_code(receipt.card_id.as_deref()),
                optional_code(receipt.receipt_tool.as_deref()),
                optional_code(receipt.strength.as_deref()),
                if receipt.issues.is_empty() {
                    "-".to_string()
                } else {
                    markdown_cell(&receipt.issues.join("; "))
                }
            ));
        }
        out.push('\n');
    }
    out.push_str("## Trust boundary\n\n");
    out.push_str(&report.trust_boundary);
    out.push('\n');
    out
}

fn optional_code(value: Option<&str>) -> String {
    match value {
        Some(value) if !value.is_empty() => format!("`{}`", markdown_cell(value)),
        _ => "-".to_string(),
    }
}

fn markdown_cell(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::receipts::{ReceiptAuditEntry, ReceiptAuditReport, ReceiptAuditSummary};

    #[test]
    fn receipt_audit_markdown_escapes_table_cells() {
        let report = ReceiptAuditReport {
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            mode: "receipt-audit".to_string(),
            policy: "advisory".to_string(),
            audit_date: "2026-05-20".to_string(),
            trust_boundary: "Static witness receipt audit only; not memory-safety proof."
                .to_string(),
            summary: ReceiptAuditSummary {
                receipts: 1,
                invalid: 1,
                ..ReceiptAuditSummary::default()
            },
            receipts: vec![ReceiptAuditEntry {
                path: "receipts/miri|focused.json".to_string(),
                card_id: Some("UR-pipe|card-c1".to_string()),
                receipt_tool: Some("miri|nightly".to_string()),
                strength: Some("ran|odd".to_string()),
                expires_at: None,
                statuses: vec!["invalid|metadata".to_string()],
                issues: vec!["receipt tool `miri|nightly` is not routed".to_string()],
                matched_card: None,
                route_tools: Vec::new(),
            }],
        };

        let markdown = render_markdown(&report);

        assert!(markdown.contains("invalid\\|metadata"));
        assert!(markdown.contains("`receipts/miri\\|focused.json`"));
        assert!(markdown.contains("`UR-pipe\\|card-c1`"));
        assert!(markdown.contains("`miri\\|nightly`"));
        assert!(markdown.contains("`ran\\|odd`"));
        assert!(markdown.contains("miri\\|nightly"));
        assert!(!markdown.contains("`UR-pipe|card-c1`"));
    }
}
