use std::fmt;
use std::path::Path;
use std::process::Command as ProcessCommand;

use crate::command::{CheckOptions, DiffInput};
use serde_json::json;
use unsafe_review_core::{AnalyzeOutput, ManualCandidate, Scope};

pub(super) fn print_first_pr_report(
    output: &AnalyzeOutput,
    out_dir: &Path,
    root: &Path,
    check: &CheckOptions,
    manual_candidates: &[ManualCandidate],
    no_changed_gaps_message: &str,
    no_changed_gaps_limitation: &str,
    artifacts: &[&str],
) {
    print_first_pr_overview(output, out_dir);
    print_manual_candidate_handoff(out_dir, root, manual_candidates);
    print_receipt_audit_handoff(check);
    print_top_card_summary(
        output,
        root,
        no_changed_gaps_message,
        no_changed_gaps_limitation,
    );
    print_artifact_paths(out_dir, artifacts);
    print_trust_boundary();
}

fn print_receipt_audit_handoff(check: &CheckOptions) {
    println!("Audit saved receipts:");
    println!("  {}", receipt_audit_command(check));
    println!("  saved receipt metadata only; unsafe-review did not run a witness");
}

fn print_manual_candidate_handoff(
    out_dir: &Path,
    root: &Path,
    manual_candidates: &[ManualCandidate],
) {
    println!("Manual candidates:");
    println!(
        "  {} (manual/advisory; not analyzer ReviewCards)",
        out_dir.join("manual-candidates.json").display()
    );
    println!("  Count: {}", manual_candidates.len());
    if let Some(candidate) = manual_candidates.first() {
        println!("  First manual candidate: {}", candidate.id);
        println!("  Explain: {}", explain_command(root, &candidate.id));
        println!("  Agent packet: {}", context_command(root, &candidate.id));
        println!(
            "  Witness plan: {}",
            candidate_witness_plan_command(root, &candidate.id)
        );
    }
    println!(
        "  manual candidates are advisory manual targets, not analyzer-discovered, not policy inputs, and unsafe-review did not run witnesses"
    );
}

fn receipt_audit_command(check: &CheckOptions) -> String {
    let mut parts = vec![
        "unsafe-review".to_string(),
        "receipt".to_string(),
        "audit".to_string(),
        "--root".to_string(),
        shell_arg(&check.root.display().to_string()),
    ];
    if let Some(base) = &check.base {
        parts.push("--base".to_string());
        parts.push(shell_arg(base));
    }
    if let Some(diff) = &check.diff {
        parts.push("--diff".to_string());
        match diff {
            DiffInput::File(path) => parts.push(shell_arg(&path.display().to_string())),
            DiffInput::Stdin => parts.push("-".to_string()),
        }
    }
    if let Some(max_cards) = check.max_cards {
        parts.push("--max-cards".to_string());
        parts.push(max_cards.to_string());
    }
    parts.push("--format".to_string());
    parts.push("markdown".to_string());
    parts.join(" ")
}

fn shell_arg(value: &str) -> String {
    if value.chars().any(char::is_whitespace) {
        format!("\"{}\"", value.replace('"', "\\\""))
    } else {
        value.to_string()
    }
}

fn print_first_pr_overview(output: &AnalyzeOutput, out_dir: &Path) {
    println!("unsafe-review first-pr");
    println!("unsafe-review wrote an advisory PR bundle.");
    println!("- Artifact directory: {}", out_dir.display());
    println!("- Review cards: {}", output.summary.cards);
    println!(
        "- Open actionable gaps: {}",
        output.summary.open_actionable_gaps
    );
    println!("Open:");
    println!("  {}", out_dir.join("pr-summary.md").display());
    println!("Agent repair queue:");
    println!(
        "  {} (copy-only; unsafe-review did not run an agent)",
        out_dir.join("repair-queue.json").display()
    );
}

fn print_top_card_summary(
    output: &AnalyzeOutput,
    root: &Path,
    no_changed_gaps_message: &str,
    no_changed_gaps_limitation: &str,
) {
    if output.summary.open_actionable_gaps == 0 {
        println!("{no_changed_gaps_message}");
        println!("{no_changed_gaps_limitation}");
        return;
    }

    let Some(card) = output.cards.first() else {
        return;
    };

    println!("Top card:");
    println!(
        "  {}:{} `{}`",
        card.site.location.file.display(),
        card.site.location.line,
        card.operation.family.as_str()
    );
    println!("  Class: `{}`", card.class.as_str());
    if !card.missing.is_empty() {
        let missing = card
            .missing
            .iter()
            .map(|missing| missing.kind.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        println!("  Missing: {missing}");
    }
    if let Some(route) = card.routes.first() {
        println!("  Route: `{}`", route.kind.as_str());
    }
    println!("  Next: {}", card.next_action.summary);
    println!("Explain top card:");
    println!("  {}", explain_command(root, &card.id));
    println!("Agent packet:");
    println!("  {}", context_command(root, &card.id));
}

fn explain_command(root: &Path, card_id: &impl fmt::Display) -> String {
    format!(
        "unsafe-review explain --root {} {card_id}",
        shell_arg(&root.display().to_string())
    )
}

fn context_command(root: &Path, card_id: &impl fmt::Display) -> String {
    format!(
        "unsafe-review context --root {} {card_id} --json",
        shell_arg(&root.display().to_string())
    )
}

fn candidate_witness_plan_command(root: &Path, candidate_id: &str) -> String {
    format!(
        "unsafe-review candidate witness-plan --root {} {}",
        shell_arg(&root.display().to_string()),
        shell_arg(candidate_id)
    )
}

pub(super) fn render_review_kit_manifest(
    output: &AnalyzeOutput,
    root: &Path,
    check: &CheckOptions,
    manual_candidates: &[ManualCandidate],
    artifacts: &[&str],
) -> String {
    let value = json!({
        "schema_version": "0.1",
        "tool": "unsafe-review",
        "tool_version": env!("CARGO_PKG_VERSION"),
        "mode": "review_kit_manifest",
        "source": "first_pr",
        "policy": output.policy.as_str(),
        "scope": scope_name(&output.scope),
        "base_ref": check.base.as_deref(),
        "head_commit": git_head_commit(root),
        "summary": {
            "cards": output.summary.cards,
            "open_actionable_gaps": output.summary.open_actionable_gaps,
        },
        "top_card_id": output.cards.first().map(|card| card.id.to_string()),
        "handoff": review_kit_handoff(output, root, check, manual_candidates),
        "artifacts": artifacts
            .iter()
            .map(|path| artifact_entry(path))
            .collect::<Vec<_>>(),
        "trust_boundary": "Static unsafe contract review kit manifest only; this indexes first-pr artifacts and does not reclassify ReviewCards. It is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, and not site-execution proof. unsafe-review did not run witnesses, post comments, edit source, run an agent, or enforce blocking policy.",
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|err| {
        format!("{{\n  \"error\": \"review kit serialization failed: {err}\"\n}}")
    })
}

fn review_kit_handoff(
    output: &AnalyzeOutput,
    root: &Path,
    check: &CheckOptions,
    manual_candidates: &[ManualCandidate],
) -> serde_json::Value {
    let top_card = output.cards.first().map(|card| {
        json!({
            "card_id": card.id.to_string(),
            "explain": explain_command(root, &card.id),
            "context_json": context_command(root, &card.id),
        })
    });

    json!({
        "reviewer_summary": "pr-summary.md",
        "receipt_audit_markdown": receipt_audit_command(check),
        "top_card": top_card,
        "manual_candidates": review_kit_manual_candidate_handoff(manual_candidates, root),
        "trust_boundary": "Copy-only review-kit handoff commands; unsafe-review did not run witnesses, run agents, post comments, edit source, or enforce blocking policy.",
    })
}

fn review_kit_manual_candidate_handoff(
    manual_candidates: &[ManualCandidate],
    root: &Path,
) -> serde_json::Value {
    let first_candidate = manual_candidates.first().map(|candidate| {
        json!({
            "id": candidate.id.as_str(),
            "source": "manual",
            "manual_candidate": true,
            "analyzer_discovered": false,
            "explain": explain_command(root, &candidate.id),
            "context_json": context_command(root, &candidate.id),
            "witness_plan": candidate_witness_plan_command(root, &candidate.id),
        })
    });

    json!({
        "artifact": "manual-candidates.json",
        "manual_candidates": manual_candidates.len(),
        "analyzer_discovered": 0,
        "first_candidate": first_candidate,
        "trust_boundary": "Manual/advisory candidate handoff only; manual candidates are not analyzer-discovered ReviewCards, not policy inputs, and not witness execution. Receipts against manual candidates attach external evidence to the manual candidate ID only and do not import ReviewCard witness evidence.",
    })
}

pub(super) fn render_manual_candidates_artifact(candidates: &[ManualCandidate]) -> String {
    let candidate_values = candidates
        .iter()
        .map(manual_candidate_artifact_entry)
        .collect::<Vec<_>>();
    let evidence_refs = candidates
        .iter()
        .map(|candidate| candidate.evidence.len())
        .sum::<usize>();
    let value = json!({
        "schema_version": "manual-candidates/v1",
        "tool": "unsafe-review",
        "tool_version": env!("CARGO_PKG_VERSION"),
        "mode": "manual_candidate_index",
        "source": "first_pr",
        "summary": {
            "manual_candidates": candidates.len(),
            "external_evidence_refs": evidence_refs,
            "analyzer_discovered": 0,
        },
        "candidates": candidate_values,
        "reviewcard_artifact_relationship": {
            "cards.json": "ReviewCard-only analyzer output; manual candidates are listed only in manual-candidates.json.",
            "cards.sarif": "ReviewCard-only analyzer output; manual candidates are not emitted as SARIF analyzer results.",
            "comment-plan.json": "ReviewCard-only comment planning; manual candidates are not selected for automatic comment plans.",
            "lsp.json": "ReviewCard-only saved editor projection; manual candidates are not emitted as analyzer diagnostics.",
            "repair-queue.json": "ReviewCard-only repair queue; manual candidates are not automatic repair tasks.",
            "receipt-audit.md": "Receipts may match manual candidate IDs as manual/advisory targets without importing them as ReviewCard witness evidence.",
            "policy-report": "ReviewCard-only policy simulation; manual candidates are not policy gating inputs."
        },
        "trust_boundary": "Manual/advisory static unsafe contract review candidate index only; candidates are not analyzer-discovered ReviewCards, not a proof of UB, not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution proof, not repository safety, and not policy gating. unsafe-review did not run witnesses, post comments, edit source, run an agent, or enforce blocking policy.",
    });
    let mut rendered = serde_json::to_string_pretty(&value).unwrap_or_else(|err| {
        format!("{{\n  \"error\": \"manual candidate artifact serialization failed: {err}\"\n}}")
    });
    rendered.push('\n');
    rendered
}

fn manual_candidate_artifact_entry(candidate: &ManualCandidate) -> serde_json::Value {
    let mut value = serde_json::to_value(candidate).unwrap_or_else(|_| json!({}));
    if let Some(object) = value.as_object_mut() {
        object.insert("analyzer_discovered".to_string(), json!(false));
        object.insert(
            "location_text".to_string(),
            json!(format!(
                "{}:{}",
                candidate.location.file.display(),
                candidate.location.line
            )),
        );
        object.insert(
            "explain_command".to_string(),
            json!(format!("unsafe-review explain {}", candidate.id)),
        );
        object.insert(
            "context_command".to_string(),
            json!(format!("unsafe-review context {} --json", candidate.id)),
        );
        object.insert(
            "witness_plan_command".to_string(),
            json!(format!(
                "unsafe-review candidate witness-plan {}",
                candidate.id
            )),
        );
    }
    value
}

fn scope_name(scope: &Scope) -> &'static str {
    match scope {
        Scope::Diff => "diff",
        Scope::Repo => "repo",
    }
}

fn git_head_commit(root: &Path) -> Option<String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--verify")
        .arg("HEAD")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn artifact_entry(path: &str) -> serde_json::Value {
    json!({
        "path": path,
        "kind": artifact_kind(path),
        "format": artifact_format(path),
        "schema_version": artifact_schema_version(path),
    })
}

fn artifact_kind(path: &str) -> &'static str {
    match path {
        "review-kit.json" => "review_kit_manifest",
        "cards.json" => "review_cards",
        "pr-summary.md" => "reviewer_summary",
        "github-summary.md" => "github_summary",
        "cards.sarif" => "sarif",
        "comment-plan.json" => "comment_plan",
        "witness-plan.md" => "witness_plan",
        "receipt-audit.md" => "receipt_audit",
        "manual-candidates.json" => "manual_candidates",
        "lsp.json" => "saved_lsp",
        "repair-queue.json" => "repair_queue",
        _ => "unknown",
    }
}

fn artifact_format(path: &str) -> &'static str {
    if path.ends_with(".json") {
        "json"
    } else if path.ends_with(".md") {
        "markdown"
    } else if path.ends_with(".sarif") {
        "sarif"
    } else {
        "unknown"
    }
}

fn artifact_schema_version(path: &str) -> Option<&'static str> {
    match path {
        "review-kit.json" | "cards.json" | "comment-plan.json" | "lsp.json"
        | "repair-queue.json" => Some("0.1"),
        "manual-candidates.json" => Some("manual-candidates/v1"),
        "cards.sarif" => Some("2.1.0"),
        _ => None,
    }
}

fn print_artifact_paths(out_dir: &Path, artifacts: &[&str]) {
    println!("Artifacts:");
    for name in artifacts {
        println!("  {}", out_dir.join(name).display());
    }
}

fn print_trust_boundary() {
    println!("Trust boundary:");
    println!(
        "  static unsafe contract review only; not memory-safety proof, not UB-free status, and not Miri-clean status."
    );
    println!(
        "  unsafe-review did not run witnesses, post comments, edit source, or enforce blocking policy."
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_card_handoff_commands_quote_roots_with_spaces() {
        let root = Path::new("C:/Code/Rust With Spaces/unsafe-review");
        let card_id = "UR-fixture-src-lib-rs-owner-operation-read-hash-hazard-c1";

        assert_eq!(
            explain_command(root, &card_id),
            "unsafe-review explain --root \"C:/Code/Rust With Spaces/unsafe-review\" UR-fixture-src-lib-rs-owner-operation-read-hash-hazard-c1"
        );
        assert_eq!(
            context_command(root, &card_id),
            "unsafe-review context --root \"C:/Code/Rust With Spaces/unsafe-review\" UR-fixture-src-lib-rs-owner-operation-read-hash-hazard-c1 --json"
        );
    }

    #[test]
    fn review_kit_manifest_lists_artifacts_and_boundary() -> Result<(), String> {
        let output = AnalyzeOutput {
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            root: Path::new(".").to_path_buf(),
            scope: Scope::Diff,
            mode: unsafe_review_core::AnalysisMode::Draft,
            policy: unsafe_review_core::PolicyMode::Advisory,
            summary: unsafe_review_core::api::Summary {
                cards: 0,
                open_actionable_gaps: 0,
                ..Default::default()
            },
            cards: Vec::new(),
        };
        let check = CheckOptions {
            root: Path::new("fixtures/safe_code_no_cards").to_path_buf(),
            base: Some("origin/main".to_string()),
            diff: None,
            format: crate::command::Format::Human,
            policy: unsafe_review_core::PolicyMode::Advisory,
            out: None,
            max_cards: None,
        };
        let rendered = render_review_kit_manifest(
            &output,
            Path::new("fixtures/safe_code_no_cards"),
            &check,
            &[],
            &["review-kit.json", "cards.json", "pr-summary.md"],
        );
        let value: serde_json::Value = match serde_json::from_str(&rendered) {
            Ok(value) => value,
            Err(err) => return Err(format!("manifest should render JSON: {err}")),
        };

        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["mode"], "review_kit_manifest");
        assert_eq!(value["scope"], "diff");
        assert_eq!(value["base_ref"], "origin/main");
        assert!(value["top_card_id"].is_null());
        assert_eq!(value["handoff"]["reviewer_summary"], "pr-summary.md");
        assert!(
            value["handoff"]["receipt_audit_markdown"]
                .as_str()
                .unwrap_or("")
                .contains("unsafe-review receipt audit --root fixtures/safe_code_no_cards")
        );
        assert!(
            value["handoff"]["receipt_audit_markdown"]
                .as_str()
                .unwrap_or("")
                .contains("--format markdown")
        );
        assert!(value["handoff"]["top_card"].is_null());
        assert_eq!(
            value["handoff"]["manual_candidates"]["artifact"],
            "manual-candidates.json"
        );
        assert_eq!(
            value["handoff"]["manual_candidates"]["manual_candidates"],
            0
        );
        assert_eq!(
            value["handoff"]["manual_candidates"]["analyzer_discovered"],
            0
        );
        assert!(value["handoff"]["manual_candidates"]["first_candidate"].is_null());
        assert!(
            value["handoff"]["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("did not run witnesses")
        );
        assert_eq!(value["artifacts"][0]["path"], "review-kit.json");
        assert_eq!(value["artifacts"][1]["schema_version"], "0.1");
        assert!(value["artifacts"][2]["schema_version"].is_null());
        assert!(
            value["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("did not run witnesses")
        );
        Ok(())
    }
}
