use crate::analysis::receipts::{
    ReceiptAuditCard, ReceiptAuditManualCandidate, ReceiptAuditReport,
};

pub(crate) fn render_json(report: &ReceiptAuditReport) -> String {
    match serde_json::to_string_pretty(report) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"receipt audit serialization failed: {err}\"\n}}"),
    }
}

pub(crate) fn render_markdown(report: &ReceiptAuditReport) -> String {
    let mut out = String::new();
    out.push_str("# unsafe-review receipt audit\n\n");
    out.push_str(
        "Static audit of saved receipt metadata against current ReviewCards and manual candidates.\n\n",
    );
    out.push_str(&format!("Audit date: `{}`\n\n", report.audit_date));
    out.push_str("## Summary\n\n");
    out.push_str("| Receipts | Matched | Unmatched | Expired | Stale | Wrong identity | Wrong tool | Weaker than route | Command hash mismatch | Duplicate | Invalid |\n");
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
    out.push_str("## Reviewer front panel\n\n");
    out.push_str(&format!(
        "- Matched receipt metadata: {}\n",
        report.summary.matched
    ));
    out.push_str(&format!(
        "- Receipts imported as current witness evidence: {}\n",
        witness_importable_count(report)
    ));
    out.push_str(&format!(
        "- Receipts imported as current reach evidence: {}\n",
        reach_importable_count(report)
    ));
    out.push_str(&format!(
        "- Receipts without a current card match: {} unmatched, {} stale\n",
        report.summary.unmatched, report.summary.stale
    ));
    out.push_str("- Problem flags: ");
    let problem_flags = problem_flags(&report.summary);
    if problem_flags.is_empty() {
        out.push_str("none\n");
    } else {
        out.push_str(&problem_flags.join("; "));
        out.push('\n');
    }
    if problem_flags.is_empty() {
        out.push_str(
            "- Next action: keep matching receipt metadata attached to the review record.\n",
        );
    } else {
        out.push_str(
            "- Next action: review nonzero problem flags before treating saved receipts as current evidence.\n",
        );
    }
    out.push_str(
        "- Boundary: matched witness receipts improve witness evidence only; matched external integration reach receipts improve reach evidence only. They do not erase missing contracts, guards, or unrelated evidence gaps.\n\n",
    );
    out.push_str("## Receipts\n\n");
    if report.receipts.is_empty() {
        out.push_str("No receipt files found.\n\n");
    } else {
        out.push_str(
            "| Status | Receipt | Card | Matched target | Tool | Strength | Verdict | Summary | Author | Recorded | Expires | Command hash | Limitations | Routed tools | Issues |\n",
        );
        out.push_str("|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|\n");
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

fn problem_flags(summary: &crate::analysis::receipts::ReceiptAuditSummary) -> Vec<String> {
    [
        ("unmatched", summary.unmatched),
        ("stale", summary.stale),
        ("expired", summary.expired),
        ("wrong identity", summary.wrong_identity),
        ("wrong tool", summary.wrong_tool),
        ("weaker than route", summary.weaker_than_required),
        ("command hash mismatch", summary.command_hash_mismatch),
        ("duplicate", summary.duplicate),
        ("invalid", summary.invalid),
    ]
    .into_iter()
    .filter(|(_label, count)| *count > 0)
    .map(|(label, count)| format!("{label}: {count}"))
    .collect()
}

fn witness_importable_count(report: &ReceiptAuditReport) -> usize {
    report
        .receipts
        .iter()
        .filter(|receipt| {
            receipt
                .statuses
                .iter()
                .any(|status| status == "imports_witness_evidence")
        })
        .count()
}

fn reach_importable_count(report: &ReceiptAuditReport) -> usize {
    report
        .receipts
        .iter()
        .filter(|receipt| {
            receipt
                .statuses
                .iter()
                .any(|status| status == "imports_reach_evidence")
        })
        .count()
}

fn receipt_row(receipt: &crate::analysis::receipts::ReceiptAuditEntry) -> String {
    format!(
        "| {} | `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
        markdown_cell(&receipt.statuses.join(", ")),
        receipt.path,
        optional_code(receipt.card_id.as_deref()),
        matched_target(
            receipt.matched_card.as_ref(),
            receipt.matched_manual_candidate.as_ref(),
        ),
        optional_code(receipt.receipt_tool.as_deref()),
        optional_code(receipt.strength.as_deref()),
        verdict_cell(receipt.verdict.as_deref()),
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

fn verdict_cell(verdict: Option<&str>) -> String {
    match verdict {
        Some("not_reproduced") => "`not_reproduced` (single run; not a safety claim)".to_string(),
        Some(verdict) if !verdict.is_empty() => format!("`{}`", markdown_cell(verdict)),
        _ => "-".to_string(),
    }
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

fn matched_target(
    card: Option<&ReceiptAuditCard>,
    candidate: Option<&ReceiptAuditManualCandidate>,
) -> String {
    if let Some(candidate) = candidate {
        return manual_candidate_target(candidate);
    }
    if let Some(card) = card {
        return format!(
            "`{}` / `{}` / proof path `{}` / source `{}` / `{}` / {} missing; confirmation {}; next: {}",
            card.class_name,
            card.operation_family,
            card.proof_path,
            card.source,
            markdown_cell(&card.operation),
            card.missing_count,
            confirmation_state_label(&card.confirmation_state),
            markdown_cell(&card.next_action)
        );
    }
    "-".to_string()
}

fn manual_candidate_target(candidate: &ReceiptAuditManualCandidate) -> String {
    let mut parts = vec![
        format!(
            "`manual_candidate` / `{}` / source `manual` / `{}` / {}",
            candidate.operation_family,
            markdown_cell(&candidate.operation),
            markdown_cell(&candidate.location)
        ),
        format!("route: {}", markdown_cell(&candidate.safe_caller)),
        format!("invariant: {}", markdown_cell(&candidate.invariant)),
    ];
    if let Some(proof_mode) = &candidate.proof_mode {
        parts.push(format!(
            "proof mode: `{}` / system Bun `{}`",
            markdown_cell(&proof_mode.kind),
            markdown_cell(&proof_mode.system_bun_expected)
        ));
    }
    if let Some(oracle_map) = &candidate.oracle_map {
        parts.push(format!(
            "oracle: `{}` `{}` / `{}` / confidence `{}` / limitation {}",
            markdown_cell(&oracle_map.oracle_language),
            markdown_cell(&oracle_map.oracle_path.display().to_string()),
            markdown_cell(&oracle_map.oracle_kind),
            markdown_cell(&oracle_map.coverage_confidence),
            markdown_cell(&oracle_map.limitation)
        ));
    }
    if let Some(fix_boundary) = &candidate.fix_boundary {
        parts.push(format!("fix boundary: {}", markdown_cell(fix_boundary)));
    }
    if let Some(pr_aperture) = &candidate.pr_aperture {
        parts.push(format!("PR aperture: {}", markdown_cell(pr_aperture)));
    }
    if let Some(fix) = candidate.fix_options.first() {
        parts.push(format!("first fix: {}", markdown_cell(fix)));
    }
    if let Some(target) = candidate.test_targets.first() {
        parts.push(format!("first test: `{}`", markdown_cell(target)));
    }
    if let Some(note) = candidate.do_not_touch.first() {
        parts.push(format!("first do-not-touch: {}", markdown_cell(note)));
    }
    if let Some(evidence) = candidate.evidence.first() {
        let mut evidence_parts = vec![format!("first evidence: `{}`", evidence.kind)];
        if let Some(path) = &evidence.path {
            evidence_parts.push(format!("path `{}`", markdown_cell(path)));
        }
        if let Some(command) = &evidence.command {
            evidence_parts.push(format!("command `{}`", markdown_cell(command)));
        }
        if let Some(limitation) = &evidence.limitation {
            evidence_parts.push(format!("limitation {}", markdown_cell(limitation)));
        }
        parts.push(evidence_parts.join(", "));
    }
    parts.push(format!("next: {}", markdown_cell(&candidate.next_action)));
    parts.push(format!(
        "boundary: {}",
        markdown_cell(&candidate.trust_boundary)
    ));
    parts.join("; ")
}

fn confirmation_state_label(state: &str) -> String {
    if state == "not_reproduced" {
        return "`not_reproduced` (single run; not a safety claim)".to_string();
    }
    format!("`{}`", markdown_cell(state))
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
            trust_boundary: "Static receipt audit only; does not execute witnesses or external tests and does not independently prove site reach.".to_string(),
            limitations: vec![
                "audits saved receipt metadata only".to_string(),
                "does not execute Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, Crux, or external integration tests".to_string(),
                "matched witness receipts improve witness evidence only and do not erase missing contracts, guards, or reach evidence".to_string(),
                "matched external integration reach receipts improve reach evidence only and do not erase missing contracts, guards, or witness evidence".to_string(),
            ],
            summary: ReceiptAuditSummary {
                receipts: 3,
                matched: 2,
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
                    verdict: None,
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
                    matched_manual_candidate: None,
                    route_tools: Vec::new(),
                },
                ReceiptAuditEntry {
                    path: ".unsafe-review/receipts/miri-ran.json".to_string(),
                    card_id: Some(
                        "UR-live-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                            .to_string(),
                    ),
                    receipt_tool: Some("miri".to_string()),
                    strength: Some("ran".to_string()),
                    verdict: Some("not_reproduced".to_string()),
                    summary: Some("focused witness passed".to_string()),
                    author: Some("core/fixtures".to_string()),
                    recorded_at: Some("2026-05-20T00:00:00Z".to_string()),
                    expires_at: Some("2026-08-18".to_string()),
                    command_hash: Some("3e163b0bce29ff2e".to_string()),
                    limitations: vec!["fixture only".to_string()],
                    statuses: vec![
                        "imports_witness_evidence".to_string(),
                        "matched".to_string(),
                    ],
                    issues: Vec::new(),
                    matched_card: Some(ReceiptAuditCard {
                        id: "UR-live-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                            .to_string(),
                        class_name: "guard_missing".to_string(),
                        operation: "unsafe { ptr.cast::<Header>().read() }".to_string(),
                        operation_family: "raw_pointer_read".to_string(),
                        proof_path: "source_route_only".to_string(),
                        confirmation_state: "executed".to_string(),
                        missing_count: 2,
                        next_action: "Add or expose guard | witness\nThen attach receipt"
                            .to_string(),
                        source: "analyzer".to_string(),
                        manual_candidate: false,
                        analyzer_discovered: true,
                    }),
                    matched_manual_candidate: None,
                    route_tools: vec!["miri".to_string(), "cargo-careful".to_string()],
                },
                ReceiptAuditEntry {
                    path: ".unsafe-review/receipts/weak-wrong-tool.json".to_string(),
                    card_id: Some(
                        "UR-live-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
                            .to_string(),
                    ),
                    receipt_tool: Some("loom".to_string()),
                    strength: Some("configured".to_string()),
                    verdict: None,
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
                        class_name: "guard_missing".to_string(),
                        operation: "unsafe { ptr.cast::<Header>().read() }".to_string(),
                        operation_family: "raw_pointer_read".to_string(),
                        proof_path: "source_route_only".to_string(),
                        confirmation_state: "executed".to_string(),
                        missing_count: 2,
                        next_action: "Add or expose guard | witness\nThen attach receipt"
                            .to_string(),
                        source: "analyzer".to_string(),
                        manual_candidate: false,
                        analyzer_discovered: true,
                    }),
                    matched_manual_candidate: None,
                    route_tools: vec!["miri".to_string(), "cargo-careful".to_string()],
                },
            ],
        };

        let markdown = render_markdown(&report);

        assert!(markdown.contains("# unsafe-review receipt audit"));
        assert!(markdown.contains("Audit date: `2026-05-26`"));
        assert!(markdown.contains("Command hash mismatch"));
        assert!(markdown.contains("| 3 | 2 | 1 | 0 | 1 | 0 | 1 | 1 | 1 | 0 | 0 |"));
        assert!(markdown.contains("## Reviewer front panel"));
        assert!(markdown.contains("- Matched receipt metadata: 2"));
        assert!(markdown.contains("- Receipts imported as current witness evidence: 1"));
        assert!(markdown.contains("- Receipts imported as current reach evidence: 0"));
        assert!(markdown.contains("- Receipts without a current card match: 1 unmatched, 1 stale"));
        assert!(markdown.contains(
            "- Problem flags: unmatched: 1; stale: 1; wrong tool: 1; weaker than route: 1; command hash mismatch: 1"
        ));
        assert!(markdown.contains(
            "review nonzero problem flags before treating saved receipts as current evidence"
        ));
        assert!(markdown.contains("matched witness receipts improve witness evidence only"));
        assert!(
            markdown.contains(
                "matched external integration reach receipts improve reach evidence only"
            )
        );
        assert!(markdown.contains(
            "| Status | Receipt | Card | Matched target | Tool | Strength | Verdict | Summary | Author | Recorded | Expires | Command hash | Limitations | Routed tools | Issues |"
        ));
        assert!(markdown.contains("`not_reproduced` (single run; not a safety claim)"));
        assert!(markdown.contains("missing; confirmation `executed`; next:"));
        assert!(markdown.contains("focused witness"));
        assert!(markdown.contains("focused witness passed"));
        assert!(markdown.contains("`core/fixtures`"));
        assert!(markdown.contains("`2026-05-20T00:00:00Z`"));
        assert!(markdown.contains("`2026-08-18`"));
        assert!(markdown.contains("imports_witness_evidence, matched"));
        assert!(markdown.contains("`4ce9d7c8eeb19a30`"));
        assert!(markdown.contains("`3e163b0bce29ff2e`"));
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
        assert!(markdown.contains(
            "`guard_missing` / `raw_pointer_read` / proof path `source_route_only` / source `analyzer`"
        ));
        assert!(markdown.contains("unsafe { ptr.cast::<Header>().read() }"));
        assert!(markdown.contains("Add or expose guard \\| witness Then attach receipt"));
        assert!(markdown.contains("## Limitations"));
        assert!(markdown.contains("does not execute Miri"));
        assert!(markdown.contains("improve witness evidence only"));
        assert!(markdown.contains("improve reach evidence only"));
        assert!(markdown.contains("## Trust boundary"));
        assert!(markdown.contains("does not execute witnesses"));
        assert!(markdown.contains("does not independently prove site reach"));
    }

    #[test]
    fn markdown_projects_manual_candidate_receipt_targets() {
        let report =
            ReceiptAuditReport {
                schema_version: "0.1".to_string(),
                tool: "unsafe-review".to_string(),
                mode: "receipt-audit".to_string(),
                policy: "advisory".to_string(),
                audit_date: "2026-05-26".to_string(),
                trust_boundary: "Static witness receipt audit only; does not execute witnesses."
                    .to_string(),
                limitations: vec![
                "manual candidate receipts attach external evidence to that manual candidate only"
                    .to_string(),
            ],
                summary: ReceiptAuditSummary {
                    receipts: 1,
                    matched: 1,
                    ..ReceiptAuditSummary::default()
                },
                receipts: vec![ReceiptAuditEntry {
                path: ".unsafe-review/receipts/manual.json".to_string(),
                card_id: Some("R4R2-S001".to_string()),
                receipt_tool: Some("human-deep-review".to_string()),
                strength: Some("test_targeted".to_string()),
                verdict: None,
                summary: Some("manual route reviewed".to_string()),
                author: Some("core/fixtures".to_string()),
                recorded_at: Some("2026-05-20T00:00:00Z".to_string()),
                expires_at: Some("2026-08-18".to_string()),
                command_hash: None,
                limitations: vec!["manual evidence only".to_string()],
                statuses: vec!["manual_candidate".to_string(), "matched".to_string()],
                issues: Vec::new(),
                matched_card: None,
                matched_manual_candidate: Some(ReceiptAuditManualCandidate {
                    id: "R4R2-S001".to_string(),
                    title: "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes"
                        .to_string(),
                    location: "src/runtime/webcore/TextDecoder.rs:237".to_string(),
                    operation: "core::slice::from_raw_parts".to_string(),
                    operation_family: "raw_pointer_read".to_string(),
                    safe_caller: "TextDecoder.decode SharedArrayBuffer route".to_string(),
                    invariant: "&[u8] memory must not be concurrently mutated".to_string(),
                    oracle_map: None,
                    proof_mode: None,
                    fix_boundary: None,
                    pr_aperture: None,
                    evidence: vec![crate::analysis::receipts::ReceiptAuditManualCandidateEvidence {
                        kind: "runtime_witness".to_string(),
                        path: Some(
                            "target/unsafe-scout/textdecoder-shared-race-route.out".to_string(),
                        ),
                        summary: Some("Bun route reaches shared backing bytes".to_string()),
                        command: Some(
                            "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
                                .to_string(),
                        ),
                        limitation: Some(
                            "runtime route evidence only; not memory-safety proof".to_string(),
                        ),
                    }],
                    fix_options: vec![
                        "Copy SharedArrayBuffer-backed bytes into stable owned storage".to_string(),
                    ],
                    test_targets: vec![
                        "test/js/webcore/textdecoder-sharedarraybuffer.test.ts".to_string(),
                    ],
                    do_not_touch: vec![
                        "Do not rewrite unrelated TextDecoder encoding paths".to_string(),
                    ],
                    next_action:
                        "Review the manual candidate and preserve receipts as external evidence"
                            .to_string(),
                    trust_boundary:
                        "manual candidate; not analyzer-discovered; not witness execution; not proof of memory safety; not UB-free status; not Miri-clean status; not site-execution proof; not policy readiness"
                            .to_string(),
                    source: "manual".to_string(),
                    manual_candidate: true,
                    analyzer_discovered: false,
                }),
                route_tools: Vec::new(),
            }],
            };

        let markdown = render_markdown(&report);

        assert!(markdown.contains("manual_candidate, matched"));
        assert!(markdown.contains("`manual_candidate` / `raw_pointer_read` / source `manual`"));
        assert!(markdown.contains("src/runtime/webcore/TextDecoder.rs:237"));
        assert!(markdown.contains("route: TextDecoder.decode SharedArrayBuffer route"));
        assert!(markdown.contains("invariant: &[u8] memory must not be concurrently mutated"));
        assert!(markdown.contains("first fix: Copy SharedArrayBuffer-backed bytes"));
        assert!(
            markdown
                .contains("first test: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`")
        );
        assert!(
            markdown.contains(
                "first do-not-touch: Do not rewrite unrelated TextDecoder encoding paths"
            )
        );
        assert!(markdown.contains("first evidence: `runtime_witness`"));
        assert!(markdown.contains("runtime route evidence only; not memory-safety proof"));
        assert!(markdown.contains("manual candidate; not analyzer-discovered"));
        assert!(!markdown.contains("imports_witness_evidence"));
    }
}
