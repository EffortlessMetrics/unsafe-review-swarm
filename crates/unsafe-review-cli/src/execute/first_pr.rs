use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::path::Path;
use std::process::Command as ProcessCommand;

use crate::command::{CheckOptions, DiffInput};
use serde_json::json;
use unsafe_review_core::{
    AnalyzeOutput, ManualCandidate, ReviewCard, Scope, manual_candidate_implementer_handoff,
    render_repair_queue,
};

const MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT: usize = 5;
const REVIEW_CARD_REVIEW_KIT_QUEUE_LIMIT: usize = 5;
const REVIEW_CARD_REPAIR_QUEUE_BUCKETS: [&str; 6] = [
    "repairable_by_guard",
    "repairable_by_safety_docs",
    "repairable_by_test",
    "requires_witness_receipt",
    "requires_human_review",
    "do_not_auto_repair",
];

pub(super) struct FirstPrReport<'a> {
    pub(super) output: &'a AnalyzeOutput,
    pub(super) out_dir: &'a Path,
    pub(super) root: &'a Path,
    pub(super) check: &'a CheckOptions,
    pub(super) manual_candidates: &'a [ManualCandidate],
    pub(super) no_changed_gaps_message: &'a str,
    pub(super) no_changed_gaps_limitation: &'a str,
    pub(super) artifacts: &'a [&'a str],
}

pub(super) fn print_first_pr_report(report: FirstPrReport<'_>) {
    print_first_pr_overview(report.output, report.out_dir);
    print_manual_candidate_handoff(report.out_dir, report.root, report.manual_candidates);
    print_receipt_audit_handoff(report.check);
    print_top_card_summary(
        report.output,
        report.root,
        report.no_changed_gaps_message,
        report.no_changed_gaps_limitation,
    );
    print_artifact_paths(report.out_dir, report.artifacts);
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
        if let Some(summary) = manual_candidate_guidance_summary(candidate) {
            println!("  Guidance: {summary}");
        }
        if let Some(target) = candidate.test_targets.first() {
            println!("  First test target: {target}");
        }
        println!("  Explain: {}", explain_command(root, &candidate.id));
        println!("  Agent packet: {}", context_command(root, &candidate.id));
        println!(
            "  Witness plan: {}",
            candidate_witness_plan_command(root, &candidate.id)
        );
    }
    print_manual_candidate_queue_preview(root, manual_candidates);
    println!(
        "  Review-kit candidate queue: first {} of {} manual candidate(s)",
        manual_candidates
            .len()
            .min(MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT),
        manual_candidates.len()
    );
    println!(
        "  manual candidates are advisory manual targets, not analyzer-discovered, not policy inputs, and unsafe-review did not run witnesses"
    );
}

fn print_manual_candidate_queue_preview(root: &Path, manual_candidates: &[ManualCandidate]) {
    let queue_len = manual_candidates
        .len()
        .min(MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT);
    println!(
        "  Manual candidate queue preview: first {queue_len} of {} manual candidate(s)",
        manual_candidates.len()
    );
    for candidate in manual_candidates.iter().take(queue_len) {
        println!(
            "    - {} at {} ({}) evidence refs: {}",
            candidate.id,
            manual_candidate_location_text(candidate),
            candidate.operation_family,
            candidate.evidence.len()
        );
        if let Some((label, value)) = manual_candidate_first_guidance_cue(candidate) {
            println!("      {label}: {value}");
        }
        println!(
            "      Agent packet: {}",
            context_command(root, &candidate.id)
        );
        println!(
            "      Witness plan: {}",
            candidate_witness_plan_command(root, &candidate.id)
        );
    }
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
        card_path_display(&card.site.location.file),
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
            "changed_files": output.summary.changed_files,
            "changed_rust_files": output.summary.changed_rust_files,
            "changed_non_rust_files": output.summary.changed_non_rust_files,
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
        "review_cards": review_kit_review_card_handoff(output, root),
        "manual_candidates": review_kit_manual_candidate_handoff(manual_candidates, root),
        "trust_boundary": "Copy-only review-kit handoff commands; unsafe-review did not run witnesses, run agents, post comments, edit source, or enforce blocking policy.",
    })
}

fn review_kit_review_card_handoff(output: &AnalyzeOutput, root: &Path) -> serde_json::Value {
    let repair_queue = review_kit_repair_queue_index(output);
    let card_queue = output
        .cards
        .iter()
        .take(REVIEW_CARD_REVIEW_KIT_QUEUE_LIMIT)
        .map(|card| review_kit_review_card_queue_entry(card, root, repair_queue.get(&card.id.0)))
        .collect::<Vec<_>>();
    let omitted_cards = output.cards.len().saturating_sub(card_queue.len());

    json!({
        "artifact": "cards.json",
        "repair_queue_artifact": "repair-queue.json",
        "review_cards": output.cards.len(),
        "card_queue_limit": REVIEW_CARD_REVIEW_KIT_QUEUE_LIMIT,
        "card_queue": card_queue,
        "omitted_cards": omitted_cards,
        "trust_boundary": "Static unsafe contract review only; copy-only ReviewCard queue preview projected from cards.json and repair-queue.json. It does not run agents, run witnesses, edit source, post comments, suppress cards, resolve cards, or enforce blocking policy. It is not a proof of memory safety, not UB-free status, not a Miri result, not Miri-clean status, not site-execution proof, not repair success, and not policy readiness.",
    })
}

fn review_kit_review_card_queue_entry(
    card: &ReviewCard,
    root: &Path,
    repair_queue: Option<&ReviewKitRepairQueueProjection>,
) -> serde_json::Value {
    let path = card_path_display(&card.site.location.file);
    let missing_evidence = card
        .missing
        .iter()
        .map(|missing| missing.message.as_str())
        .collect::<Vec<_>>();
    let repair_queue_buckets = repair_queue
        .map(|projection| projection.buckets.clone())
        .unwrap_or_default();
    let repair_queue_bucket_reasons = repair_queue
        .map(|projection| projection.bucket_reasons.clone())
        .unwrap_or_default();
    let agent_readiness = repair_queue
        .map(|projection| projection.agent_readiness.clone())
        .unwrap_or_else(|| {
            json!({
                "ready": false,
                "state": "requires_human_review",
                "reasons": ["missing repair-queue projection"],
            })
        });

    json!({
        "card_id": card.id.to_string(),
        "source": "review_card",
        "class": card.class.as_str(),
        "priority": card.priority.as_str(),
        "confidence": card.confidence.as_str(),
        "path": path,
        "line": card.site.location.line,
        "location_text": format!("{}:{}", card_path_display(&card.site.location.file), card.site.location.line),
        "operation_family": card.operation.family.as_str(),
        "operation": card.operation.expression.as_str(),
        "missing_evidence": missing_evidence,
        "next_action": card.next_action.summary.as_str(),
        "verify_commands": &card.next_action.verify_commands,
        "witness_routes": review_kit_witness_routes(card),
        "repair_queue_buckets": repair_queue_buckets,
        "repair_queue_bucket_reasons": repair_queue_bucket_reasons,
        "agent_readiness": agent_readiness,
        "explain": explain_command(root, &card.id),
        "context_json": context_command(root, &card.id),
        "trust_boundary": "Static unsafe contract review only; copy-only ReviewCard queue entry projected from cards.json and repair-queue.json; it is not a proof of memory safety, not UB-free status, not a Miri result, and not site-execution proof. unsafe-review did not run agents, run witnesses, edit source, post comments, suppress cards, resolve cards, or enforce blocking policy.",
    })
}

fn review_kit_witness_routes(card: &ReviewCard) -> Vec<serde_json::Value> {
    card.routes
        .iter()
        .map(|route| {
            json!({
                "kind": route.kind.as_str(),
                "reason": route.reason.as_str(),
                "command": route.command.as_deref(),
                "required": route.required,
            })
        })
        .collect()
}

#[derive(Clone)]
struct ReviewKitRepairQueueProjection {
    buckets: Vec<String>,
    bucket_reasons: Vec<String>,
    agent_readiness: serde_json::Value,
}

fn review_kit_repair_queue_index(
    output: &AnalyzeOutput,
) -> BTreeMap<String, ReviewKitRepairQueueProjection> {
    let Ok(repair_queue) = serde_json::from_str::<serde_json::Value>(&render_repair_queue(output))
    else {
        return BTreeMap::new();
    };
    let Some(buckets) = repair_queue
        .get("buckets")
        .and_then(serde_json::Value::as_object)
    else {
        return BTreeMap::new();
    };

    let mut index = BTreeMap::<String, ReviewKitRepairQueueProjection>::new();
    for bucket in REVIEW_CARD_REPAIR_QUEUE_BUCKETS {
        let Some(entries) = buckets.get(bucket).and_then(serde_json::Value::as_array) else {
            continue;
        };
        for entry in entries {
            let Some(card_id) = entry.get("card_id").and_then(serde_json::Value::as_str) else {
                continue;
            };
            let projection = index.entry(card_id.to_string()).or_insert_with(|| {
                ReviewKitRepairQueueProjection {
                    buckets: Vec::new(),
                    bucket_reasons: Vec::new(),
                    agent_readiness: entry
                        .get("agent_readiness")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                }
            });
            if !projection
                .buckets
                .iter()
                .any(|candidate| candidate == bucket)
            {
                projection.buckets.push(bucket.to_string());
            }
            if let Some(reason) = entry
                .get("bucket_reason")
                .and_then(serde_json::Value::as_str)
            {
                if !projection
                    .bucket_reasons
                    .iter()
                    .any(|candidate| candidate == reason)
                {
                    projection.bucket_reasons.push(reason.to_string());
                }
            }
            if projection.agent_readiness.is_null() {
                projection.agent_readiness = entry
                    .get("agent_readiness")
                    .cloned()
                    .unwrap_or(serde_json::Value::Null);
            }
        }
    }
    index
}

fn review_kit_manual_candidate_handoff(
    manual_candidates: &[ManualCandidate],
    root: &Path,
) -> serde_json::Value {
    let first_candidate = manual_candidates
        .first()
        .map(|candidate| review_kit_manual_candidate_queue_entry(candidate, root));
    let candidate_queue = manual_candidates
        .iter()
        .take(MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT)
        .map(|candidate| review_kit_manual_candidate_queue_entry(candidate, root))
        .collect::<Vec<_>>();
    let omitted_candidates = manual_candidates
        .len()
        .saturating_sub(candidate_queue.len());

    json!({
        "artifact": "manual-candidates.json",
        "manual_candidates": manual_candidates.len(),
        "analyzer_discovered": 0,
        "first_candidate": first_candidate,
        "candidate_queue_limit": MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT,
        "candidate_queue": candidate_queue,
        "omitted_candidates": omitted_candidates,
        "trust_boundary": "Manual/advisory candidate handoff only; manual candidates are not analyzer-discovered ReviewCards, not policy inputs, and not witness execution. Receipts against manual candidates attach external evidence to the manual candidate ID only and do not import ReviewCard witness evidence.",
    })
}

fn review_kit_manual_candidate_queue_entry(
    candidate: &ManualCandidate,
    root: &Path,
) -> serde_json::Value {
    json!({
        "id": candidate.id.as_str(),
        "source": "manual",
        "manual_candidate": true,
        "analyzer_discovered": false,
        "title": candidate.title.as_str(),
        "location_text": format!(
            "{}:{}",
            candidate.location.file.display(),
            candidate.location.line
        ),
        "operation_family": candidate.operation_family.as_str(),
        "evidence_refs": candidate.evidence.len(),
        "implementer_handoff": manual_candidate_implementer_handoff(candidate),
        "explain": explain_command(root, &candidate.id),
        "context_json": context_command(root, &candidate.id),
        "witness_plan": candidate_witness_plan_command(root, &candidate.id),
    })
}

pub(super) fn render_first_pr_front_door_artifact(
    artifact_name: &str,
    rendered: String,
    root: &Path,
    manual_candidates: &[ManualCandidate],
) -> String {
    if manual_candidates.is_empty() {
        return rendered;
    }

    match artifact_name {
        "pr-summary.md" => insert_before_section(
            rendered,
            "## Card table",
            &render_manual_candidate_front_panel(root, manual_candidates),
        ),
        "github-summary.md" => insert_before_section(
            rendered,
            "## Open next",
            &render_manual_candidate_front_panel(root, manual_candidates),
        ),
        "witness-plan.md" => insert_before_section(
            rendered,
            "## Trust boundary",
            &render_manual_candidate_witness_follow_up(root, manual_candidates),
        ),
        _ => rendered,
    }
}

fn insert_before_section(rendered: String, heading: &str, section: &str) -> String {
    if let Some(index) = rendered.find(heading) {
        let mut out = String::with_capacity(rendered.len() + section.len());
        out.push_str(&rendered[..index]);
        if !out.ends_with("\n\n") {
            out.push('\n');
        }
        out.push_str(section);
        out.push_str(&rendered[index..]);
        out
    } else {
        let mut out = rendered;
        if !out.ends_with("\n\n") {
            out.push_str("\n\n");
        }
        out.push_str(section);
        out
    }
}

fn render_manual_candidate_front_panel(
    root: &Path,
    manual_candidates: &[ManualCandidate],
) -> String {
    let mut out = String::new();
    out.push_str("## Manual candidates\n\n");
    out.push_str(&format!(
        "- Imported manual candidates: {} (manual/advisory; not analyzer-discovered ReviewCards)\n",
        manual_candidates.len()
    ));
    if let Some(candidate) = manual_candidates.first() {
        out.push_str(&format!(
            "- First manual candidate: `{}` at `{}` (`{}`)\n",
            candidate.id,
            manual_candidate_location_text(candidate),
            candidate.operation_family
        ));
        out.push_str(&format!("- Safe caller route: {}\n", candidate.safe_caller));
        out.push_str(&format!("- Invariant at risk: {}\n", candidate.invariant));
        out.push_str(&format!(
            "- External evidence refs: {}\n",
            candidate.evidence.len()
        ));
        append_manual_candidate_guidance_lines(&mut out, candidate);
        out.push_str(&format!(
            "- Explain: `{}`\n",
            explain_command(root, &candidate.id)
        ));
        out.push_str(&format!(
            "- Agent context: `{}`\n",
            context_command(root, &candidate.id)
        ));
        out.push_str(&format!(
            "- Witness plan: `{}`\n",
            candidate_witness_plan_command(root, &candidate.id)
        ));
    }
    append_manual_candidate_queue_preview(&mut out, root, manual_candidates);
    out.push_str("- Manual candidate index: `manual-candidates.json`; candidates stay out of ReviewCard-only outputs.\n");
    out.push_str("- Boundary: copy-only manual handoff; unsafe-review did not discover these candidates, did not run witnesses, did not edit source, or make them policy inputs.\n\n");
    out
}

fn render_manual_candidate_witness_follow_up(
    root: &Path,
    manual_candidates: &[ManualCandidate],
) -> String {
    let mut out = String::new();
    out.push_str("## Manual candidate witness follow-up\n\n");
    let _ = writeln!(
        &mut out,
        "- Imported manual candidates: {} (manual/advisory; not analyzer-discovered ReviewCards)",
        manual_candidates.len()
    );
    if let Some(candidate) = manual_candidates.first() {
        let _ = writeln!(
            &mut out,
            "- First manual candidate: `{}` at `{}` (`{}`)",
            candidate.id,
            manual_candidate_location_text(candidate),
            candidate.operation_family
        );
        let _ = writeln!(&mut out, "- Safe caller route: {}", candidate.safe_caller);
        let _ = writeln!(&mut out, "- Invariant at risk: {}", candidate.invariant);
        let _ = writeln!(
            &mut out,
            "- External evidence refs: {}",
            candidate.evidence.len()
        );
        append_manual_candidate_guidance_lines(&mut out, candidate);
        let _ = writeln!(
            &mut out,
            "- Full manual witness plan: `{}`",
            candidate_witness_plan_command(root, &candidate.id)
        );
        let _ = writeln!(
            &mut out,
            "- Agent context: `{}`",
            context_command(root, &candidate.id)
        );
    }
    append_manual_candidate_queue_preview(&mut out, root, manual_candidates);
    out.push_str("- Manual candidate index: `manual-candidates.json`; candidates stay out of ReviewCard-only witness route groups.\n");
    out.push_str("- Receipt boundary: manual candidate receipts attach external evidence to the manual candidate ID only; they do not import ReviewCard witness evidence.\n");
    out.push_str("- Boundary: copy-only manual follow-up; unsafe-review did not discover these candidates, did not run witnesses, did not edit source, or make them policy inputs.\n\n");
    out
}

fn append_manual_candidate_guidance_lines(out: &mut String, candidate: &ManualCandidate) {
    if let Some(summary) = manual_candidate_guidance_summary(candidate) {
        let _ = writeln!(out, "- Guidance: {summary}");
    }
    if let Some(option) = candidate.fix_options.first() {
        let _ = writeln!(out, "- First fix option: {option}");
    }
    if let Some(target) = candidate.test_targets.first() {
        let _ = writeln!(out, "- First test target: `{target}`");
    }
    if let Some(note) = candidate.do_not_touch.first() {
        let _ = writeln!(out, "- First do-not-touch note: {note}");
    }
}

fn append_manual_candidate_queue_preview(
    out: &mut String,
    root: &Path,
    manual_candidates: &[ManualCandidate],
) {
    let queue_len = manual_candidates
        .len()
        .min(MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT);
    let _ = writeln!(
        out,
        "- Manual candidate queue preview: first {queue_len} of {} manual candidate(s)",
        manual_candidates.len()
    );
    for candidate in manual_candidates.iter().take(queue_len) {
        let _ = write!(
            out,
            "  - `{}` at `{}` (`{}`); evidence refs: {}",
            candidate.id,
            manual_candidate_location_text(candidate),
            candidate.operation_family,
            candidate.evidence.len()
        );
        if let Some((label, value)) = manual_candidate_first_guidance_cue(candidate) {
            let _ = write!(out, "; {label}: `{value}`");
        }
        out.push('\n');
        let _ = writeln!(
            out,
            "    - Agent context: `{}`",
            context_command(root, &candidate.id)
        );
        let _ = writeln!(
            out,
            "    - Witness plan: `{}`",
            candidate_witness_plan_command(root, &candidate.id)
        );
    }
}

fn manual_candidate_first_guidance_cue(
    candidate: &ManualCandidate,
) -> Option<(&'static str, &str)> {
    if let Some(value) = candidate.test_targets.first() {
        return Some(("first test target", value.as_str()));
    }
    if let Some(value) = candidate.fix_options.first() {
        return Some(("first fix option", value.as_str()));
    }
    if let Some(value) = candidate.do_not_touch.first() {
        return Some(("first do-not-touch note", value.as_str()));
    }
    None
}

fn manual_candidate_guidance_summary(candidate: &ManualCandidate) -> Option<String> {
    let total =
        candidate.fix_options.len() + candidate.test_targets.len() + candidate.do_not_touch.len();
    if total == 0 {
        return None;
    }
    Some(format!(
        "{} fix option(s), {} test target(s), {} do-not-touch note(s)",
        candidate.fix_options.len(),
        candidate.test_targets.len(),
        candidate.do_not_touch.len()
    ))
}

fn manual_candidate_location_text(candidate: &ManualCandidate) -> String {
    format!(
        "{}:{}",
        candidate.location.file.display(),
        candidate.location.line
    )
}

pub(super) fn render_manual_candidates_artifact(
    root: &Path,
    candidates: &[ManualCandidate],
) -> String {
    let candidate_values = candidates
        .iter()
        .map(|candidate| manual_candidate_artifact_entry(root, candidate))
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

fn manual_candidate_artifact_entry(root: &Path, candidate: &ManualCandidate) -> serde_json::Value {
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
            json!(explain_command(root, &candidate.id)),
        );
        object.insert(
            "context_command".to_string(),
            json!(context_command(root, &candidate.id)),
        );
        object.insert(
            "witness_plan_command".to_string(),
            json!(candidate_witness_plan_command(root, &candidate.id)),
        );
        object.insert(
            "implementer_handoff".to_string(),
            manual_candidate_implementer_handoff(candidate),
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

fn card_path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
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
                changed_files: 3,
                changed_rust_files: 1,
                changed_non_rust_files: 2,
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
        assert_eq!(value["summary"]["changed_files"], 3);
        assert_eq!(value["summary"]["changed_rust_files"], 1);
        assert_eq!(value["summary"]["changed_non_rust_files"], 2);
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
        assert_eq!(value["handoff"]["review_cards"]["artifact"], "cards.json");
        assert_eq!(
            value["handoff"]["review_cards"]["repair_queue_artifact"],
            "repair-queue.json"
        );
        assert_eq!(value["handoff"]["review_cards"]["review_cards"], 0);
        assert_eq!(
            value["handoff"]["review_cards"]["card_queue_limit"],
            REVIEW_CARD_REVIEW_KIT_QUEUE_LIMIT
        );
        assert_eq!(value["handoff"]["review_cards"]["omitted_cards"], 0);
        assert!(
            value["handoff"]["review_cards"]["card_queue"]
                .as_array()
                .is_some_and(|queue| queue.is_empty())
        );
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
        assert_eq!(
            value["handoff"]["manual_candidates"]["candidate_queue_limit"],
            MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT
        );
        assert_eq!(
            value["handoff"]["manual_candidates"]["omitted_candidates"],
            0
        );
        assert!(
            value["handoff"]["manual_candidates"]["candidate_queue"]
                .as_array()
                .is_some_and(|queue| queue.is_empty())
        );
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

    #[test]
    fn first_pr_front_doors_surface_manual_candidate_handoff() -> Result<(), String> {
        let candidate = manual_candidate_fixture()?;
        let root = Path::new("fixtures/bun fork");

        let github_summary = render_first_pr_front_door_artifact(
            "github-summary.md",
            "## unsafe-review advisory summary\n\n## Top card\n\n## Open next\n\n".to_string(),
            root,
            std::slice::from_ref(&candidate),
        );
        assert!(github_summary.contains("## Manual candidates"));
        assert!(github_summary.contains(
            "- Imported manual candidates: 1 (manual/advisory; not analyzer-discovered ReviewCards)"
        ));
        assert!(github_summary.contains(
            "- First manual candidate: `R4R2-S001` at `src/runtime/webcore/TextDecoder.rs:237` (`raw_pointer_read`)"
        ));
        assert!(
            github_summary
                .contains("- Safe caller route: TextDecoder.decode SharedArrayBuffer route")
        );
        assert!(
            github_summary
                .contains("- Invariant at risk: &[u8] memory must not be concurrently mutated")
        );
        assert!(github_summary.contains("- External evidence refs: 1"));
        assert!(
            github_summary
                .contains("- Guidance: 1 fix option(s), 1 test target(s), 1 do-not-touch note(s)")
        );
        assert!(github_summary.contains(
            "- First fix option: Copy SharedArrayBuffer-backed bytes before constructing the slice"
        ));
        assert!(github_summary.contains(
            "- First test target: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`"
        ));
        assert!(
            github_summary
                .contains("- First do-not-touch note: Do not rewrite unrelated TextDecoder paths")
        );
        assert!(
            github_summary
                .contains("- Manual candidate queue preview: first 1 of 1 manual candidate(s)")
        );
        assert!(github_summary.contains(
            "`R4R2-S001` at `src/runtime/webcore/TextDecoder.rs:237` (`raw_pointer_read`); evidence refs: 1; first test target: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`"
        ));
        assert!(
            github_summary.contains("unsafe-review explain --root \"fixtures/bun fork\" R4R2-S001")
        );
        assert!(
            github_summary
                .contains("unsafe-review context --root \"fixtures/bun fork\" R4R2-S001 --json")
        );
        assert!(github_summary.contains(
            "unsafe-review candidate witness-plan --root \"fixtures/bun fork\" R4R2-S001"
        ));
        assert!(github_summary.contains("candidates stay out of ReviewCard-only outputs"));
        assert!(
            github_summary
                .find("## Manual candidates")
                .ok_or_else(|| "manual candidate section should exist".to_string())?
                < github_summary
                    .find("## Open next")
                    .ok_or_else(|| "open next section should exist".to_string())?
        );

        let pr_summary = render_first_pr_front_door_artifact(
            "pr-summary.md",
            "## Top card\n\n## Card table\n\n".to_string(),
            root,
            std::slice::from_ref(&candidate),
        );
        assert!(pr_summary.contains("## Manual candidates"));
        assert!(
            pr_summary
                .find("## Manual candidates")
                .ok_or_else(|| "manual candidate section should exist".to_string())?
                < pr_summary
                    .find("## Card table")
                    .ok_or_else(|| "card table section should exist".to_string())?
        );

        let cards_json = "{}".to_string();
        assert_eq!(
            render_first_pr_front_door_artifact(
                "cards.json",
                cards_json.clone(),
                root,
                &[candidate]
            ),
            cards_json
        );

        Ok(())
    }

    fn manual_candidate_fixture() -> Result<ManualCandidate, String> {
        ManualCandidate::from_json_str(
            r#"{
              "schema_version": "manual-candidate/v1",
              "id": "R4R2-S001",
              "title": "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes",
              "location": {
                "file": "src/runtime/webcore/TextDecoder.rs",
                "line": 237
              },
              "operation_family": "raw_pointer_read",
              "unsafe_operation": "core::slice::from_raw_parts",
              "invariant": "&[u8] memory must not be concurrently mutated",
              "safe_caller": "TextDecoder.decode SharedArrayBuffer route",
              "fix_options": [
                "Copy SharedArrayBuffer-backed bytes before constructing the slice"
              ],
              "test_targets": [
                "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
              ],
              "do_not_touch": [
                "Do not rewrite unrelated TextDecoder paths"
              ],
              "evidence": [{
                "kind": "runtime_witness",
                "path": "target/unsafe-scout/textdecoder-shared-race-route.out",
                "summary": "Bun TextDecoder route reaches shared backing bytes through safe JS",
                "command": "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts",
                "limitation": "runtime route evidence only; not memory-safety proof and not analyzer-discovered"
              }],
              "trust_boundary": "manual candidate; not analyzer-discovered; not proof of repository safety"
            }"#,
        )
    }
}
