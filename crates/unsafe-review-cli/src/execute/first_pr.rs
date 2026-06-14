use std::collections::BTreeMap;
use std::fmt::{self, Write as _};
use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;

use crate::command::{CheckOptions, DiffInput};
use serde_json::json;
use unsafe_review_core::{
    AnalyzeOutput, ManualCandidate, ReviewCard, Scope, manual_candidate_implementer_handoff,
    project_review_card_confirmation, render_repair_queue,
};

const MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT: usize = 5;
const MANUAL_CANDIDATE_GITHUB_QUEUE_LIMIT: usize = 1;
const REVIEW_CARD_REVIEW_KIT_QUEUE_LIMIT: usize = 5;
const TOKMD_PACKET_PRESETS: [&str; 5] = [
    "bun-ub-handoff",
    "bun-ub-pr-body",
    "bun-ub-ledger-note",
    "bun-ub-review-map",
    "bun-ub-next-pick",
];
const STABLE_BYTE_SEED_LEDGER_PATH: &str = "docs/dogfood/stable-byte-follow-up-seeds.md";
const STABLE_BYTE_SEED_LEDGER_HEADER: [&str; 11] = [
    "Seed ID",
    "Ledger state",
    "Candidate family",
    "Surface",
    "Manual candidate",
    "Safe JS caller",
    "Rust/native sink",
    "Proof mode",
    "Suggested first PR",
    "Owner lane",
    "Triage labels",
];
const STABLE_BYTE_SOURCE_CLASSES: [&str; 6] = [
    "stable-byte-source-rab-async",
    "stable-byte-source-sab-race",
    "stable-byte-source-getter-reentry",
    "stable-byte-source-helper-dependent",
    "stable-byte-source-pathlike-live-view",
    "stable-byte-source-native-ffi-read",
];
const REVIEW_CARD_REPAIR_QUEUE_BUCKETS: [&str; 6] = [
    "repairable_by_guard",
    "repairable_by_safety_docs",
    "repairable_by_test",
    "requires_witness_receipt",
    "requires_human_review",
    "do_not_auto_repair",
];
const MANUAL_REPAIR_QUEUE_BUCKET: &str = "manual_candidate_handoff";
const MANUAL_REPAIR_QUEUE_BUCKET_REASON: &str = "manual_candidate_copy_only";
const MANUAL_REPAIR_QUEUE_ENTRY_TRUST_BOUNDARY: &str = "Copy-only manual candidate repair queue entry; not analyzer-discovered, not automatic repair, not witness execution, not source editing, not proof, and not policy gating.";

#[derive(Clone, Debug)]
struct StableByteSeed {
    seed_id: String,
    ledger_state: String,
    candidate_family: String,
    surface: String,
    manual_candidate: String,
    safe_js_caller: String,
    rust_native_sink: String,
    proof_mode: String,
    suggested_first_pr: String,
    owner_lane: String,
    triage_labels: Vec<String>,
}

#[derive(Clone, Debug)]
struct StableByteSeedLedger {
    path: &'static str,
    present: bool,
    parse_error: Option<String>,
    rows: usize,
    by_candidate_id: BTreeMap<String, StableByteSeed>,
}

pub(super) struct FirstPrReport<'a> {
    pub(super) output: &'a AnalyzeOutput,
    pub(super) out_dir: &'a Path,
    pub(super) root: &'a Path,
    pub(super) check: &'a CheckOptions,
    pub(super) manual_candidates: &'a [ManualCandidate],
    pub(super) no_changed_gaps_message: &'a str,
    pub(super) no_changed_gaps_limitation: &'a str,
    pub(super) artifacts: &'a [&'a str],
    /// Total bytes written to all artifact files in `out_dir` for this run.
    /// Diagnostic only — not a coverage claim, proof, UB-free, Miri-clean,
    /// site-execution, or performance guarantee.
    pub(super) output_bytes: u64,
}

pub(super) fn print_first_pr_report(report: FirstPrReport<'_>) {
    print_first_pr_overview(report.output, report.out_dir, report.output_bytes);
    print_manual_candidate_handoff(report.out_dir, report.root, report.manual_candidates);
    print_receipt_audit_handoff(report.check);
    print_policy_report_handoff(report.out_dir);
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

fn print_policy_report_handoff(out_dir: &Path) {
    println!("Policy report:");
    println!("  {}", artifact_path_display(out_dir, "policy-report.md"));
    println!("  ReviewCard-only policy simulation; manual candidates are not policy inputs");
}

fn print_manual_candidate_handoff(
    out_dir: &Path,
    root: &Path,
    manual_candidates: &[ManualCandidate],
) {
    let stable_byte_seed_ledger = load_stable_byte_seed_ledger(root);
    println!("Manual candidates:");
    println!(
        "  {} (manual/advisory; not analyzer ReviewCards)",
        artifact_path_display(out_dir, "manual-candidates.json")
    );
    println!("  Count: {}", manual_candidates.len());
    println!(
        "  Operation families: {}",
        render_count_map(&manual_candidate_operation_family_counts(manual_candidates))
    );
    println!(
        "  Evidence kinds: {}",
        render_count_map(&manual_candidate_evidence_kind_counts(manual_candidates))
    );
    if let Some(candidate) = manual_candidates.first() {
        println!("  First manual candidate: {}", candidate.id);
        if let Some(seed) = stable_byte_seed_ledger.by_candidate_id.get(&candidate.id) {
            println!("  {}", stable_byte_seed_terminal_summary(seed));
        }
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
    print_manual_candidate_queue_preview(root, manual_candidates, &stable_byte_seed_ledger);
    println!(
        "  Review-kit candidate queue: first {} of {} manual candidate(s)",
        manual_candidates
            .len()
            .min(MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT),
        manual_candidates.len()
    );
    println!(
        "  Manual repair queue: {} (copy-only; unsafe-review did not run an agent)",
        artifact_path_display(out_dir, "manual-repair-queue.json")
    );
    println!(
        "  Tokmd packet export: {} (formatting input only; tokmd was not run)",
        artifact_path_display(out_dir, "tokmd-packets.json")
    );
    println!(
        "  manual candidates are advisory manual targets, not analyzer-discovered, not policy inputs, and unsafe-review did not run witnesses"
    );
}

fn print_manual_candidate_queue_preview(
    root: &Path,
    manual_candidates: &[ManualCandidate],
    stable_byte_seed_ledger: &StableByteSeedLedger,
) {
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
        if let Some(seed) = stable_byte_seed_ledger.by_candidate_id.get(&candidate.id) {
            println!("      {}", stable_byte_seed_terminal_summary(seed));
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

fn print_first_pr_overview(output: &AnalyzeOutput, out_dir: &Path, output_bytes: u64) {
    println!("unsafe-review first-pr");
    println!("unsafe-review wrote an advisory PR bundle.");
    println!("- Artifact directory: {}", card_path_display(out_dir));
    println!("- Review cards: {}", output.summary.cards);
    println!(
        "- Open actionable gaps: {}",
        output.summary.open_actionable_gaps
    );
    // Output bundle disk footprint — diagnostic only; not a coverage claim,
    // proof, UB-free, Miri-clean, site-execution, or performance guarantee.
    println!("- Output bundle: {output_bytes} bytes");
    println!("Open:");
    println!("  {}", artifact_path_display(out_dir, "pr-summary.md"));
    println!("Agent repair queue:");
    println!(
        "  {} (copy-only; unsafe-review did not run an agent)",
        artifact_path_display(out_dir, "repair-queue.json")
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
    let confirmation = project_review_card_confirmation(card);
    println!("  Hypothesis: {}", confirmation.hypothesis_to_confirm);
    println!("  Build/run this first: {}", confirmation.build_this_first);
    println!("  Minimal repro cue:");
    for step in &confirmation.minimal_repro_steps {
        println!("    - {step}");
    }
    println!(
        "    - Limitation: {}",
        confirmation.minimal_repro_limitation
    );
    println!("  Confirmation step: {}", confirmation.confirmation_step);
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
    let stable_byte_seed_ledger = load_stable_byte_seed_ledger(root);
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
        "handoff": review_kit_handoff(
            output,
            root,
            check,
            manual_candidates,
            &stable_byte_seed_ledger,
        ),
        "artifacts": artifacts
            .iter()
            .map(|path| artifact_entry(path))
            .collect::<Vec<_>>(),
        "trust_boundary": "Static unsafe contract review kit manifest only; this indexes first-pr artifacts and does not reclassify ReviewCards. It is not a proof of memory safety, not UB-free status, not Miri-clean status, and not a site-execution claim unless a matching witness receipt says so. unsafe-review did not run witnesses, post comments, edit source, run an agent, or enforce blocking policy.",
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
    stable_byte_seed_ledger: &StableByteSeedLedger,
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
        "manual_candidates": review_kit_manual_candidate_handoff(
            manual_candidates,
            root,
            stable_byte_seed_ledger,
        ),
        "repair_queues": review_kit_repair_queue_front_panel(output, manual_candidates),
        "trust_boundary": "Copy-only review-kit handoff commands; unsafe-review did not run witnesses, run agents, post comments, edit source, or enforce blocking policy.",
    })
}

fn review_kit_repair_queue_front_panel(
    output: &AnalyzeOutput,
    manual_candidates: &[ManualCandidate],
) -> serde_json::Value {
    let repair_queue = review_kit_repair_queue_index(output);
    json!({
        "review_card": {
            "artifact": "repair-queue.json",
            "source": "review_card",
            "cards": output.cards.len(),
            "unique_repair_queue_cards": repair_queue.len(),
            "bucket_counts": review_kit_repair_queue_bucket_counts(&repair_queue),
            "agent_ready_cards": repair_queue.values().filter(|projection| {
                projection.agent_readiness
                    .get("ready")
                    .and_then(serde_json::Value::as_bool)
                    == Some(true)
            }).count(),
        },
        "manual_candidate": {
            "artifact": "manual-repair-queue.json",
            "source": "manual_candidate",
            "manual_candidates": manual_candidates.len(),
            "queued_candidates": manual_candidates.len(),
            "bucket": MANUAL_REPAIR_QUEUE_BUCKET,
            "bucket_reason": MANUAL_REPAIR_QUEUE_BUCKET_REASON,
            "agent_handoff_state": "copy_ready",
            "automatic": false,
        },
        "separation": "ReviewCard repair queues and manual-candidate repair queues stay separate source ledgers; this front panel only places their counts side by side for reviewer and agent routing.",
        "trust_boundary": "Unified repair-queue front panel only; it does not merge manual candidates into ReviewCard repair-queue.json, does not run agents, does not run witnesses, does not edit source, does not post comments, and is not proof, repair success, or policy readiness.",
    })
}

fn review_kit_repair_queue_bucket_counts(
    repair_queue: &BTreeMap<String, ReviewKitRepairQueueProjection>,
) -> BTreeMap<String, usize> {
    let mut counts = REVIEW_CARD_REPAIR_QUEUE_BUCKETS
        .iter()
        .map(|bucket| ((*bucket).to_string(), 0usize))
        .collect::<BTreeMap<_, _>>();
    for projection in repair_queue.values() {
        for bucket in &projection.buckets {
            if let Some(count) = counts.get_mut(bucket) {
                *count += 1;
            }
        }
    }
    counts
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
        "trust_boundary": "Static unsafe contract review only; copy-only ReviewCard queue preview projected from cards.json and repair-queue.json. It does not run agents, run witnesses, edit source, post comments, suppress cards, resolve cards, or enforce blocking policy. It is not a proof of memory safety, not UB-free status, not Miri-clean status, not a site-execution claim unless a matching witness receipt says so, not repair success, and not policy readiness.",
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
        "trust_boundary": "Static unsafe contract review only; copy-only ReviewCard queue entry projected from cards.json and repair-queue.json; it is not a proof of memory safety, not UB-free status, not Miri-clean status, and not a site-execution claim unless a matching witness receipt says so. unsafe-review did not run agents, run witnesses, edit source, post comments, suppress cards, resolve cards, or enforce blocking policy.",
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
                && !projection
                    .bucket_reasons
                    .iter()
                    .any(|candidate| candidate == reason)
            {
                projection.bucket_reasons.push(reason.to_string());
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
    stable_byte_seed_ledger: &StableByteSeedLedger,
) -> serde_json::Value {
    let first_candidate = manual_candidates.first().map(|candidate| {
        review_kit_manual_candidate_queue_entry(
            candidate,
            root,
            stable_byte_seed_ledger.by_candidate_id.get(&candidate.id),
        )
    });
    let candidate_queue = manual_candidates
        .iter()
        .take(MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT)
        .map(|candidate| {
            review_kit_manual_candidate_queue_entry(
                candidate,
                root,
                stable_byte_seed_ledger.by_candidate_id.get(&candidate.id),
            )
        })
        .collect::<Vec<_>>();
    let matched_stable_byte_seeds = manual_candidates
        .iter()
        .filter(|candidate| {
            stable_byte_seed_ledger
                .by_candidate_id
                .contains_key(&candidate.id)
        })
        .count();
    let omitted_candidates = manual_candidates
        .len()
        .saturating_sub(candidate_queue.len());

    json!({
        "artifact": "manual-candidates.json",
        "manual_repair_queue_artifact": "manual-repair-queue.json",
        "manual_candidates": manual_candidates.len(),
        "analyzer_discovered": 0,
        "operation_families": manual_candidate_operation_family_counts(manual_candidates),
        "evidence_kinds": manual_candidate_evidence_kind_counts(manual_candidates),
        "proof_modes": manual_candidate_proof_mode_counts(manual_candidates),
        "stable_byte_source_classes": manual_candidate_stable_byte_class_counts(manual_candidates),
        "ledger_states": manual_candidate_ledger_state_counts(manual_candidates),
        "with_fix_options": manual_candidates.iter().filter(|candidate| !candidate.fix_options.is_empty()).count(),
        "with_test_targets": manual_candidates.iter().filter(|candidate| !candidate.test_targets.is_empty()).count(),
        "with_do_not_touch": manual_candidates.iter().filter(|candidate| !candidate.do_not_touch.is_empty()).count(),
        "with_oracle_map": manual_candidates.iter().filter(|candidate| candidate.oracle_map.is_some()).count(),
        "with_proof_mode": manual_candidates.iter().filter(|candidate| candidate.proof_mode.is_some()).count(),
        "with_fix_boundary": manual_candidates.iter().filter(|candidate| candidate.fix_boundary.is_some()).count(),
        "with_pr_aperture": manual_candidates.iter().filter(|candidate| candidate.pr_aperture.is_some()).count(),
        "with_stable_byte_seed": matched_stable_byte_seeds,
        "stable_byte_seed_source": review_kit_stable_byte_seed_source(
            stable_byte_seed_ledger,
            matched_stable_byte_seeds,
        ),
        "reviewcard_artifact_applicability": manual_candidate_reviewcard_applicability(),
        "first_candidate": first_candidate,
        "candidate_queue_limit": MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT,
        "candidate_queue": candidate_queue,
        "omitted_candidates": omitted_candidates,
        "trust_boundary": "Manual/advisory candidate handoff only; manual candidates are not analyzer-discovered ReviewCards, not policy inputs, and not witness execution. Receipts against manual candidates attach external evidence to the manual candidate ID only and do not import ReviewCard witness evidence.",
    })
}

fn review_kit_stable_byte_seed_source(
    stable_byte_seed_ledger: &StableByteSeedLedger,
    matched_stable_byte_seeds: usize,
) -> serde_json::Value {
    stable_byte_seed_source_projection(
        stable_byte_seed_ledger,
        matched_stable_byte_seeds,
        "root-local stable-byte seed ledger rows are joined to review-kit manual candidate entries by manual candidate ID",
    )
}

fn stable_byte_seed_source_projection(
    stable_byte_seed_ledger: &StableByteSeedLedger,
    matched_stable_byte_seeds: usize,
    relationship: &'static str,
) -> serde_json::Value {
    if !stable_byte_seed_ledger.present {
        return json!({
            "included": false,
            "limitation": "Root-local stable-byte seed ledger was absent; manual candidate stable_byte metadata is still projected as advisory workflow metadata only; not analyzer discovery, not witness execution, not proof, not policy readiness, and not a ReviewCard truth"
        });
    }
    if let Some(parse_error) = &stable_byte_seed_ledger.parse_error {
        return json!({
            "included": false,
            "path": stable_byte_seed_ledger.path,
            "limitation": format!("Stable-byte seed ledger was present but not imported: {parse_error}; manual candidate stable_byte metadata is still projected as advisory workflow metadata only; not analyzer discovery, not witness execution, not proof, not policy readiness, and not a ReviewCard truth")
        });
    }
    json!({
        "included": true,
        "path": stable_byte_seed_ledger.path,
        "rows": stable_byte_seed_ledger.rows,
        "matched_manual_candidates": matched_stable_byte_seeds,
        "relationship": relationship,
        "limitation": "Stable-byte seed rows are advisory workflow metadata only; not analyzer discovery, not witness execution, not proof, not policy readiness, and not a ReviewCard truth"
    })
}

fn review_kit_manual_candidate_queue_entry(
    candidate: &ManualCandidate,
    root: &Path,
    stable_byte_seed: Option<&StableByteSeed>,
) -> serde_json::Value {
    let mut value = json!({
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
    });
    if let Some(seed) = stable_byte_seed
        && let Some(object) = value.as_object_mut()
    {
        object.insert(
            "stable_byte_seed".to_string(),
            review_kit_stable_byte_seed(seed, candidate),
        );
    }
    value
}

fn review_kit_stable_byte_seed(
    seed: &StableByteSeed,
    candidate: &ManualCandidate,
) -> serde_json::Value {
    json!({
        "source": STABLE_BYTE_SEED_LEDGER_PATH,
        "seed_id": seed.seed_id.as_str(),
        "ledger_state": seed.ledger_state.as_str(),
        "candidate_family": seed.candidate_family.as_str(),
        "surface": seed.surface.as_str(),
        "manual_candidate": seed.manual_candidate.as_str(),
        "safe_js_caller": seed.safe_js_caller.as_str(),
        "rust_native_sink": seed.rust_native_sink.as_str(),
        "proof_mode": seed.proof_mode.as_str(),
        "suggested_first_pr": seed.suggested_first_pr.as_str(),
        "manual_candidate_pr_aperture": candidate.pr_aperture.as_deref(),
        "owner_lane": seed.owner_lane.as_str(),
        "triage_labels": &seed.triage_labels,
        "candidate_consistency": {
            "stable_byte_class_matches_manual_candidate": stable_byte_source_class(candidate)
                == Some(seed.candidate_family.as_str()),
            "proof_mode_matches_manual_candidate": candidate.proof_mode.as_ref()
                .map(|proof_mode| proof_mode.kind.as_str())
                == Some(seed.proof_mode.as_str()),
            "ledger_state_matches_manual_candidate": stable_byte_ledger_state(candidate)
                == Some(seed.ledger_state.as_str()),
            "safe_js_caller_matches_manual_candidate": stable_byte_source(candidate)
                == Some(seed.safe_js_caller.as_str()),
            "rust_native_sink_matches_manual_candidate": stable_byte_sink(candidate)
                == Some(seed.rust_native_sink.as_str()),
            "suggested_first_pr_has_manual_candidate_pr_aperture": !seed.suggested_first_pr.trim().is_empty()
                && candidate.pr_aperture.as_ref().is_some_and(|value| !value.trim().is_empty()),
        },
        "trust_boundary": "Stable-byte seed row is advisory workflow metadata only; not analyzer discovery, not witness execution, not proof, not UB-free status, not Miri-clean status, not site-execution proof, not policy readiness, and not a ReviewCard truth."
    })
}

pub(super) fn manual_candidate_context_seed_projection(
    root: &Path,
    candidate: &ManualCandidate,
) -> (serde_json::Value, Option<serde_json::Value>) {
    let stable_byte_seed_ledger = load_stable_byte_seed_ledger(root);
    let seed = stable_byte_seed_ledger.by_candidate_id.get(&candidate.id);
    let matched_stable_byte_seeds = usize::from(seed.is_some());
    (
        stable_byte_seed_source_projection(
            &stable_byte_seed_ledger,
            matched_stable_byte_seeds,
            "root-local stable-byte seed ledger rows are joined to manual candidate context packets by manual candidate ID",
        ),
        seed.map(|seed| review_kit_stable_byte_seed(seed, candidate)),
    )
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
    let stable_byte_seed_ledger = load_stable_byte_seed_ledger(root);

    match artifact_name {
        "pr-summary.md" => insert_before_section(
            rendered,
            "## Card table",
            &render_manual_candidate_front_panel(
                root,
                manual_candidates,
                &stable_byte_seed_ledger,
                MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT,
                false,
            ),
        ),
        "github-summary.md" => insert_before_section(
            rendered,
            "## Open next",
            &render_manual_candidate_front_panel(
                root,
                manual_candidates,
                &stable_byte_seed_ledger,
                MANUAL_CANDIDATE_GITHUB_QUEUE_LIMIT,
                true,
            ),
        ),
        "witness-plan.md" => insert_before_section(
            rendered,
            "## Trust boundary",
            &render_manual_candidate_witness_follow_up(
                root,
                manual_candidates,
                &stable_byte_seed_ledger,
            ),
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
    stable_byte_seed_ledger: &StableByteSeedLedger,
    queue_limit: usize,
    compact: bool,
) -> String {
    let mut out = String::new();
    out.push_str("## Manual candidates\n\n");
    out.push_str(&format!(
        "- Imported manual candidates: {} (manual/advisory; not analyzer-discovered ReviewCards)\n",
        manual_candidates.len()
    ));
    append_manual_candidate_summary_mix(&mut out, manual_candidates);
    if let Some(candidate) = manual_candidates.first() {
        out.push_str(&format!(
            "- First manual candidate: `{}` at `{}` (`{}`)\n",
            candidate.id,
            manual_candidate_location_text(candidate),
            candidate.operation_family
        ));
        if compact {
            append_manual_candidate_compact_lines(
                &mut out,
                candidate,
                stable_byte_seed_ledger.by_candidate_id.get(&candidate.id),
            );
        } else {
            out.push_str(&format!("- Safe caller route: {}\n", candidate.safe_caller));
            out.push_str(&format!("- Invariant at risk: {}\n", candidate.invariant));
            out.push_str(&format!(
                "- External evidence refs: {}\n",
                candidate.evidence.len()
            ));
            append_manual_candidate_guidance_lines(
                &mut out,
                candidate,
                stable_byte_seed_ledger.by_candidate_id.get(&candidate.id),
                true,
            );
            out.push_str(&format!(
                "- Explain: `{}`\n",
                explain_command(root, &candidate.id)
            ));
        }
        out.push_str(&format!(
            "- Agent context: `{}`\n",
            context_command(root, &candidate.id)
        ));
        out.push_str(&format!(
            "- Witness plan: `{}`\n",
            candidate_witness_plan_command(root, &candidate.id)
        ));
    }
    if !compact {
        append_manual_candidate_queue_preview(
            &mut out,
            root,
            manual_candidates,
            stable_byte_seed_ledger,
            queue_limit,
            true,
        );
    }
    if compact {
        out.push_str(
            "- Manual candidate index: `manual-candidates.json`; ReviewCard-only outputs clean.\n",
        );
        out.push_str("- Manual repair queue: `manual-repair-queue.json`; copy-only, separate from ReviewCard `repair-queue.json`; no agent was run.\n");
        out.push_str(
            "- Boundary: did not discover, did not run witnesses, edit source, or make policy inputs.\n\n",
        );
    } else {
        out.push_str("- Manual candidate index: `manual-candidates.json`; candidates stay out of ReviewCard-only outputs.\n");
        out.push_str("- Manual repair queue: `manual-repair-queue.json`; copy-only manual candidate repair handoff, separate from ReviewCard `repair-queue.json`; no agent was run.\n");
        out.push_str("- Boundary: copy-only manual handoff; unsafe-review did not discover these candidates, did not run witnesses, did not edit source, or make them policy inputs.\n\n");
    }
    out
}

fn append_manual_candidate_compact_lines(
    out: &mut String,
    candidate: &ManualCandidate,
    stable_byte_seed: Option<&StableByteSeed>,
) {
    append_manual_candidate_guidance_lines(out, candidate, stable_byte_seed, false);
    let _ = writeln!(
        out,
        "- Evidence refs: {}; full route and evidence packet in sidecars.",
        candidate.evidence.len()
    );
}

fn render_manual_candidate_witness_follow_up(
    root: &Path,
    manual_candidates: &[ManualCandidate],
    stable_byte_seed_ledger: &StableByteSeedLedger,
) -> String {
    let mut out = String::new();
    out.push_str("## Manual candidate witness follow-up\n\n");
    let _ = writeln!(
        &mut out,
        "- Imported manual candidates: {} (manual/advisory; not analyzer-discovered ReviewCards)",
        manual_candidates.len()
    );
    append_manual_candidate_summary_mix(&mut out, manual_candidates);
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
        append_manual_candidate_guidance_lines(
            &mut out,
            candidate,
            stable_byte_seed_ledger.by_candidate_id.get(&candidate.id),
            true,
        );
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
    append_manual_candidate_queue_preview(
        &mut out,
        root,
        manual_candidates,
        stable_byte_seed_ledger,
        MANUAL_CANDIDATE_REVIEW_KIT_QUEUE_LIMIT,
        true,
    );
    out.push_str("- Manual candidate index: `manual-candidates.json`; candidates stay out of ReviewCard-only witness route groups.\n");
    out.push_str("- Receipt boundary: manual candidate receipts attach external evidence to the manual candidate ID only; they do not import ReviewCard witness evidence.\n");
    out.push_str("- Boundary: copy-only manual follow-up; unsafe-review did not discover these candidates, did not run witnesses, did not edit source, or make them policy inputs.\n\n");
    out
}

fn append_manual_candidate_guidance_lines(
    out: &mut String,
    candidate: &ManualCandidate,
    stable_byte_seed: Option<&StableByteSeed>,
    include_details: bool,
) {
    if let Some(stable_byte) = &candidate.stable_byte {
        if include_details {
            let _ = writeln!(
                out,
                "- Stable-byte class: `{}` (observable: `{}`; proof required: `{}`; ledger state: `{}`)",
                stable_byte.class,
                stable_byte.observable,
                stable_byte.proof_required,
                stable_byte.ledger_state
            );
            let _ = writeln!(
                out,
                "- Stable-byte route: source `{}` -> sink `{}`",
                stable_byte.source, stable_byte.sink
            );
            let _ = writeln!(out, "- Stable-byte hazard: {}", stable_byte.hazard);
        } else {
            let _ = writeln!(
                out,
                "- Stable-byte class: `{}`; proof `{}`; ledger `{}`; route `{}` -> `{}`; hazard in sidecars",
                stable_byte.class,
                stable_byte.proof_required,
                stable_byte.ledger_state,
                stable_byte.source,
                stable_byte.sink
            );
        }
    }
    if let Some(seed) = stable_byte_seed {
        let _ = writeln!(out, "- {}", stable_byte_seed_markdown_summary(seed));
    }
    if let Some(proof_mode) = &candidate.proof_mode {
        let _ = writeln!(
            out,
            "- Proof mode: `{}` (system Bun expected: `{}`; mutation required: `{}`; Miri/model required: `{}`)",
            proof_mode.kind,
            proof_mode.system_bun_expected,
            proof_mode.mutation_required,
            proof_mode.miri_required
        );
    }
    if let Some(oracle_map) = &candidate.oracle_map {
        if include_details {
            let _ = writeln!(
                out,
                "- Oracle map: Rust seam `{}` -> `{}` oracle `{}` (`{}`; confidence: `{}`; limitation: {})",
                oracle_map.rust_seam,
                oracle_map.oracle_language,
                oracle_map.oracle_path.display(),
                oracle_map.oracle_kind,
                oracle_map.coverage_confidence,
                oracle_map.limitation
            );
        } else {
            let _ = writeln!(
                out,
                "- Oracle map: `{}` -> `{}` (`{}`; `{}`; limitation in sidecars)",
                oracle_map.rust_seam,
                oracle_map.oracle_path.display(),
                oracle_map.oracle_language,
                oracle_map.oracle_kind
            );
        }
    }
    if let Some(fix_boundary) = &candidate.fix_boundary {
        let _ = writeln!(out, "- Fix boundary: {fix_boundary}");
    }
    if let Some(pr_aperture) = &candidate.pr_aperture {
        let _ = writeln!(out, "- PR aperture: {pr_aperture}");
        if include_details {
            out.push_str("- Stop line: keep the PR inside this aperture; stop before source edits if the route no longer matches or the work would broaden into unrelated unsafe sites.\n");
        } else {
            out.push_str("- Stop line: keep the PR inside this aperture.\n");
        }
    }
    if let Some(summary) = manual_candidate_guidance_summary(candidate) {
        let _ = writeln!(out, "- Guidance: {summary}");
    }
    append_manual_candidate_first_guidance_cues(out, candidate);
}

fn append_manual_candidate_first_guidance_cues(out: &mut String, candidate: &ManualCandidate) {
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

fn stable_byte_seed_markdown_summary(seed: &StableByteSeed) -> String {
    format!(
        "Stable-byte seed: `{}` (owner lane: `{}`; suggested first PR: `{}`; triage: `{}`)",
        seed.seed_id,
        seed.owner_lane,
        seed.suggested_first_pr,
        seed.triage_labels.join("`, `")
    )
}

fn stable_byte_seed_terminal_summary(seed: &StableByteSeed) -> String {
    format!(
        "Stable-byte seed: {}; owner lane: {}; suggested first PR: {}; triage: {}",
        seed.seed_id,
        seed.owner_lane,
        seed.suggested_first_pr,
        seed.triage_labels.join(", ")
    )
}

fn append_manual_candidate_queue_preview(
    out: &mut String,
    root: &Path,
    manual_candidates: &[ManualCandidate],
    stable_byte_seed_ledger: &StableByteSeedLedger,
    queue_limit: usize,
    include_commands: bool,
) {
    let queue_len = manual_candidates.len().min(queue_limit);
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
        if let Some(seed) = stable_byte_seed_ledger.by_candidate_id.get(&candidate.id) {
            let _ = write!(
                out,
                "; seed: `{}`; seed owner: `{}`; next PR: `{}`; triage: `{}`",
                seed.seed_id,
                seed.owner_lane,
                seed.suggested_first_pr,
                seed.triage_labels.join("`, `")
            );
        }
        out.push('\n');
        if include_commands {
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
}

fn manual_candidate_first_guidance_cue(
    candidate: &ManualCandidate,
) -> Option<(&'static str, &str)> {
    if let Some(proof_mode) = &candidate.proof_mode {
        return Some(("proof mode", proof_mode.kind.as_str()));
    }
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

fn append_manual_candidate_summary_mix(out: &mut String, candidates: &[ManualCandidate]) {
    let _ = writeln!(
        out,
        "- Operation families: `{}`",
        render_count_map(&manual_candidate_operation_family_counts(candidates))
    );
    let _ = writeln!(
        out,
        "- Evidence kinds: `{}`",
        render_count_map(&manual_candidate_evidence_kind_counts(candidates))
    );
}

pub(super) fn manual_candidate_operation_family_counts(
    candidates: &[ManualCandidate],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for candidate in candidates {
        *counts
            .entry(candidate.operation_family.clone())
            .or_insert(0) += 1;
    }
    counts
}

pub(super) fn manual_candidate_evidence_kind_counts(
    candidates: &[ManualCandidate],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for candidate in candidates {
        for evidence in &candidate.evidence {
            *counts.entry(evidence.kind.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn manual_candidate_proof_mode_counts(candidates: &[ManualCandidate]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for candidate in candidates {
        if let Some(proof_mode) = &candidate.proof_mode {
            *counts.entry(proof_mode.kind.clone()).or_insert(0) += 1;
        }
    }
    counts
}

fn manual_candidate_stable_byte_class_counts(
    candidates: &[ManualCandidate],
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for candidate in candidates {
        if let Some(class) = stable_byte_source_class(candidate) {
            *counts.entry(class.to_string()).or_insert(0) += 1;
        }
    }
    counts
}

fn manual_candidate_ledger_state_counts(candidates: &[ManualCandidate]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for candidate in candidates {
        if let Some(ledger_state) = stable_byte_ledger_state(candidate) {
            *counts.entry(ledger_state.to_string()).or_insert(0) += 1;
        }
    }
    counts
}

pub(super) fn render_count_map(counts: &BTreeMap<String, usize>) -> String {
    if counts.is_empty() {
        return "none".to_string();
    }
    counts
        .iter()
        .map(|(key, count)| format!("{key}: {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn manual_candidate_location_text(candidate: &ManualCandidate) -> String {
    format!(
        "{}:{}",
        candidate.location.file.display(),
        candidate.location.line
    )
}

fn stable_byte_source_class(candidate: &ManualCandidate) -> Option<&str> {
    candidate
        .stable_byte
        .as_ref()
        .map(|stable_byte| stable_byte.class.as_str())
        .or_else(|| {
            STABLE_BYTE_SOURCE_CLASSES
                .iter()
                .copied()
                .find(|class| candidate.title.contains(class))
        })
}

fn stable_byte_ledger_state(candidate: &ManualCandidate) -> Option<&str> {
    candidate
        .stable_byte
        .as_ref()
        .map(|stable_byte| stable_byte.ledger_state.as_str())
}

fn stable_byte_source(candidate: &ManualCandidate) -> Option<&str> {
    candidate
        .stable_byte
        .as_ref()
        .map(|stable_byte| stable_byte.source.as_str())
}

fn stable_byte_sink(candidate: &ManualCandidate) -> Option<&str> {
    candidate
        .stable_byte
        .as_ref()
        .map(|stable_byte| stable_byte.sink.as_str())
}

fn load_stable_byte_seed_ledger(root: &Path) -> StableByteSeedLedger {
    let path = root.join(STABLE_BYTE_SEED_LEDGER_PATH);
    if !path.is_file() {
        return StableByteSeedLedger {
            path: STABLE_BYTE_SEED_LEDGER_PATH,
            present: false,
            parse_error: None,
            rows: 0,
            by_candidate_id: BTreeMap::new(),
        };
    }
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) => {
            return StableByteSeedLedger {
                path: STABLE_BYTE_SEED_LEDGER_PATH,
                present: true,
                parse_error: Some(format!("read failed: {err}")),
                rows: 0,
                by_candidate_id: BTreeMap::new(),
            };
        }
    };
    match parse_stable_byte_seed_ledger(root, &text) {
        Ok((rows, by_candidate_id)) => StableByteSeedLedger {
            path: STABLE_BYTE_SEED_LEDGER_PATH,
            present: true,
            parse_error: None,
            rows,
            by_candidate_id,
        },
        Err(err) => StableByteSeedLedger {
            path: STABLE_BYTE_SEED_LEDGER_PATH,
            present: true,
            parse_error: Some(err),
            rows: 0,
            by_candidate_id: BTreeMap::new(),
        },
    }
}

fn parse_stable_byte_seed_ledger(
    root: &Path,
    text: &str,
) -> Result<(usize, BTreeMap<String, StableByteSeed>), String> {
    let mut in_table = false;
    let mut rows = 0usize;
    let mut seed_ids = BTreeMap::<String, usize>::new();
    let mut by_candidate_id = BTreeMap::<String, StableByteSeed>::new();
    for (line_idx, line) in text.lines().enumerate() {
        if !in_table {
            if line.contains("| Seed ID |") {
                let columns = markdown_table_columns(line);
                if columns != STABLE_BYTE_SEED_LEDGER_HEADER {
                    return Err("stable-byte seed header is not recognized".to_string());
                }
                in_table = true;
            }
            continue;
        }
        if !line.trim_start().starts_with('|') {
            break;
        }
        if line.contains("|---") {
            continue;
        }
        let columns = markdown_table_columns(line);
        if columns.len() != STABLE_BYTE_SEED_LEDGER_HEADER.len() {
            return Err("stable-byte seed row has the wrong column count".to_string());
        }
        let seed = StableByteSeed {
            seed_id: markdown_code_cell_value(columns[0]),
            ledger_state: markdown_code_cell_value(columns[1]),
            candidate_family: markdown_code_cell_value(columns[2]),
            surface: markdown_code_cell_value(columns[3]),
            manual_candidate: markdown_code_cell_value(columns[4]),
            safe_js_caller: stable_byte_seed_text_cell_value(columns[5]),
            rust_native_sink: stable_byte_seed_text_cell_value(columns[6]),
            proof_mode: markdown_code_cell_value(columns[7]),
            suggested_first_pr: markdown_code_cell_value(columns[8]),
            owner_lane: markdown_code_cell_value(columns[9]),
            triage_labels: markdown_code_spans(columns[10]),
        };
        if seed.seed_id.is_empty() || seed.manual_candidate.is_empty() {
            return Err("stable-byte seed row is missing seed id or manual candidate".to_string());
        }
        if let Some(previous_line) = seed_ids.insert(seed.seed_id.clone(), line_idx + 1) {
            return Err(format!(
                "stable-byte seed row `{}` repeats seed id first seen on line {previous_line}; each stable-byte seed row must have a unique seed id",
                seed.seed_id
            ));
        }
        if let Some(candidate_id) = stable_byte_seed_manual_candidate_id(root, &seed) {
            if let Some(previous_seed) = by_candidate_id.get(&candidate_id) {
                return Err(format!(
                    "stable-byte seed rows `{}` and `{}` both resolve to manual candidate `{candidate_id}`; each manual candidate must have at most one stable-byte seed row",
                    previous_seed.seed_id, seed.seed_id
                ));
            }
            by_candidate_id.insert(candidate_id, seed);
        }
        rows += 1;
    }
    if !in_table {
        return Err("stable-byte seed table was not found".to_string());
    }
    Ok((rows, by_candidate_id))
}

fn stable_byte_seed_manual_candidate_id(root: &Path, seed: &StableByteSeed) -> Option<String> {
    let text = fs::read_to_string(root.join(&seed.manual_candidate)).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn markdown_table_columns(line: &str) -> Vec<&str> {
    let trimmed = line.trim();
    let trimmed = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let trimmed = trimmed.strip_suffix('|').unwrap_or(trimmed);
    trimmed.split('|').map(str::trim).collect()
}

fn markdown_code_cell_value(cell: &str) -> String {
    let cell = cell.trim();
    let Some(start) = cell.find('`') else {
        return cell.to_string();
    };
    let Some(end) = cell[start + 1..].find('`') else {
        return cell.to_string();
    };
    cell[start + 1..start + 1 + end].to_string()
}

fn stable_byte_seed_text_cell_value(cell: &str) -> String {
    cell.trim().replace('`', "").trim().to_string()
}

fn markdown_code_spans(cell: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut rest = cell;
    while let Some(start) = rest.find('`') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('`') else {
            break;
        };
        let value = rest[..end].trim();
        if !value.is_empty() {
            spans.push(value.to_string());
        }
        rest = &rest[end + 1..];
    }
    spans
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
    let operation_families = manual_candidate_operation_family_counts(candidates);
    let evidence_kinds = manual_candidate_evidence_kind_counts(candidates);
    let value = json!({
        "schema_version": "manual-candidates/v1",
        "tool": "unsafe-review",
        "tool_version": env!("CARGO_PKG_VERSION"),
        "mode": "manual_candidate_index",
        "source": "first_pr",
        "summary": {
            "manual_candidates": candidates.len(),
            "external_evidence_refs": evidence_refs,
            "operation_families": operation_families,
            "evidence_kinds": evidence_kinds,
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
            "receipt-audit.json": "Receipts may match manual candidate IDs as manual/advisory targets without importing them as ReviewCard witness evidence.",
            "policy-report.json": "ReviewCard-only policy simulation; manual candidates are not policy gating inputs.",
            "policy-report.md": "ReviewCard-only policy simulation; manual candidates are not policy gating inputs."
        },
        "reviewcard_artifact_applicability": manual_candidate_reviewcard_applicability(),
        "trust_boundary": "Manual/advisory static unsafe contract review candidate index only; candidates are not analyzer-discovered ReviewCards, not a proof of UB, not a proof of memory safety, not UB-free status, not Miri-clean status, not a site-execution claim unless a matching witness receipt says so, not repository safety, and not policy gating. unsafe-review did not run witnesses, post comments, edit source, run an agent, or enforce blocking policy.",
    });
    let mut rendered = serde_json::to_string_pretty(&value).unwrap_or_else(|err| {
        format!("{{\n  \"error\": \"manual candidate artifact serialization failed: {err}\"\n}}")
    });
    rendered.push('\n');
    rendered
}

pub(super) fn render_manual_repair_queue_artifact(
    root: &Path,
    candidates: &[ManualCandidate],
) -> String {
    let stable_byte_seed_ledger = load_stable_byte_seed_ledger(root);
    let queue = candidates
        .iter()
        .map(|candidate| {
            manual_repair_queue_entry(
                root,
                candidate,
                stable_byte_seed_ledger.by_candidate_id.get(&candidate.id),
            )
        })
        .collect::<Vec<_>>();
    let matched_stable_byte_seeds = candidates
        .iter()
        .filter(|candidate| {
            stable_byte_seed_ledger
                .by_candidate_id
                .contains_key(&candidate.id)
        })
        .count();
    let value = json!({
        "schema_version": "manual-repair-queue/v1",
        "tool": "unsafe-review",
        "tool_version": env!("CARGO_PKG_VERSION"),
        "mode": "manual_candidate_repair_queue",
        "source": "manual_candidate",
        "policy": "advisory",
        "summary": {
            "manual_candidates": candidates.len(),
            "queued_candidates": queue.len(),
            "analyzer_discovered": 0,
            "external_evidence_refs": candidates.iter().map(|candidate| candidate.evidence.len()).sum::<usize>(),
            "operation_families": manual_candidate_operation_family_counts(candidates),
            "evidence_kinds": manual_candidate_evidence_kind_counts(candidates),
            "proof_modes": manual_candidate_proof_mode_counts(candidates),
            "stable_byte_source_classes": manual_candidate_stable_byte_class_counts(candidates),
            "ledger_states": manual_candidate_ledger_state_counts(candidates),
            "with_fix_options": candidates.iter().filter(|candidate| !candidate.fix_options.is_empty()).count(),
            "with_test_targets": candidates.iter().filter(|candidate| !candidate.test_targets.is_empty()).count(),
            "with_do_not_touch": candidates.iter().filter(|candidate| !candidate.do_not_touch.is_empty()).count(),
            "with_oracle_map": candidates.iter().filter(|candidate| candidate.oracle_map.is_some()).count(),
            "with_proof_mode": candidates.iter().filter(|candidate| candidate.proof_mode.is_some()).count(),
            "with_fix_boundary": candidates.iter().filter(|candidate| candidate.fix_boundary.is_some()).count(),
            "with_pr_aperture": candidates.iter().filter(|candidate| candidate.pr_aperture.is_some()).count(),
            "with_stable_byte_seed": matched_stable_byte_seeds,
            "stable_byte_seed_source": stable_byte_seed_source_projection(
                &stable_byte_seed_ledger,
                matched_stable_byte_seeds,
                "root-local stable-byte seed ledger rows are joined to manual-repair-queue entries by manual candidate ID",
            ),
        },
        "queue": queue,
        "trust_boundary": "Copy-only manual candidate repair queue; entries come from imported manual candidates, not analyzer-discovered ReviewCards. This is not an automatic repair queue, not proof of memory safety, not UB-free status, not Miri-clean status, not a site-execution claim unless a matching witness receipt says so, not policy gating, and not repair success. unsafe-review did not run agents, did not run witnesses, did not edit source, did not post comments, and did not enforce blocking policy.",
    });
    let mut rendered = serde_json::to_string_pretty(&value).unwrap_or_else(|err| {
        format!("{{\n  \"error\": \"manual repair queue serialization failed: {err}\"\n}}")
    });
    rendered.push('\n');
    rendered
}

pub(super) fn render_tokmd_packets_artifact(
    root: &Path,
    candidates: &[ManualCandidate],
    comment_plan: Option<&str>,
) -> String {
    let stable_byte_seed_ledger = load_stable_byte_seed_ledger(root);
    let comment_plan_input = tokmd_comment_plan_input(comment_plan);
    let packets = candidates
        .iter()
        .map(|candidate| {
            tokmd_packet_entry(
                root,
                candidate,
                stable_byte_seed_ledger.by_candidate_id.get(&candidate.id),
                &stable_byte_seed_ledger,
            )
        })
        .collect::<Vec<_>>();
    let matched_stable_byte_seeds = candidates
        .iter()
        .filter(|candidate| {
            stable_byte_seed_ledger
                .by_candidate_id
                .contains_key(&candidate.id)
        })
        .count();
    let value = json!({
        "schema_version": "tokmd-packets/v1",
        "tool": "unsafe-review",
        "tool_version": env!("CARGO_PKG_VERSION"),
        "mode": "tokmd_packet_bundle",
        "source": "first_pr",
        "policy": "advisory",
        "renderer": {
            "tokmd_run": false,
            "available_presets": TOKMD_PACKET_PRESETS,
            "presets_status": "formatting requirements only; unsafe-review exported packet inputs but did not render tokmd output",
        },
        "summary": {
            "manual_candidates": candidates.len(),
            "packets": packets.len(),
            "analyzer_discovered": 0,
            "external_evidence_refs": candidates.iter().map(|candidate| candidate.evidence.len()).sum::<usize>(),
            "operation_families": manual_candidate_operation_family_counts(candidates),
            "evidence_kinds": manual_candidate_evidence_kind_counts(candidates),
            "with_proof_mode": candidates.iter().filter(|candidate| candidate.proof_mode.is_some()).count(),
            "with_fix_boundary": candidates.iter().filter(|candidate| candidate.fix_boundary.is_some()).count(),
            "with_pr_aperture": candidates.iter().filter(|candidate| candidate.pr_aperture.is_some()).count(),
            "with_oracle_map": candidates.iter().filter(|candidate| candidate.oracle_map.is_some()).count(),
            "with_stable_byte_source_class": candidates.iter().filter(|candidate| stable_byte_source_class(candidate).is_some()).count(),
            "with_stable_byte_seed": matched_stable_byte_seeds,
        },
        "inputs": tokmd_packet_inputs(
            &stable_byte_seed_ledger,
            matched_stable_byte_seeds,
            comment_plan_input,
        ),
        "packets": packets,
        "trust_boundary": "Tokmd-friendly packet bundle for formatting inputs only; manual/advisory candidates are not analyzer-discovered ReviewCards, not policy inputs, not a proof of UB, not a proof of memory safety, not UB-free status, not Miri-clean status, not a site-execution claim unless a matching witness receipt says so, not repair success, and not policy readiness. unsafe-review did not run tokmd, witnesses, Miri, Bun, Node, agents, post comments, edit source, or enforce blocking policy.",
    });
    let mut rendered = serde_json::to_string_pretty(&value).unwrap_or_else(|err| {
        format!("{{\n  \"error\": \"tokmd packet serialization failed: {err}\"\n}}")
    });
    rendered.push('\n');
    rendered
}

fn manual_repair_queue_entry(
    root: &Path,
    candidate: &ManualCandidate,
    stable_byte_seed: Option<&StableByteSeed>,
) -> serde_json::Value {
    let mut value = json!({
        "id": candidate.id.as_str(),
        "source": "manual",
        "manual_candidate": true,
        "analyzer_discovered": false,
        "title": candidate.title.as_str(),
        "location_text": manual_candidate_location_text(candidate),
        "operation_family": candidate.operation_family.as_str(),
        "unsafe_operation": candidate.unsafe_operation.as_str(),
        "safe_caller": candidate.safe_caller.as_str(),
        "invariant_at_risk": candidate.invariant.as_str(),
        "oracle_map": candidate.oracle_map.as_ref(),
        "external_evidence_refs": candidate.evidence.len(),
        "fix_options": &candidate.fix_options,
        "test_targets": &candidate.test_targets,
        "do_not_touch": &candidate.do_not_touch,
        "implementer_handoff": manual_candidate_implementer_handoff(candidate),
        "explain": explain_command(root, &candidate.id),
        "context_json": context_command(root, &candidate.id),
        "witness_plan": candidate_witness_plan_command(root, &candidate.id),
        "bucket": MANUAL_REPAIR_QUEUE_BUCKET,
        "bucket_reason": MANUAL_REPAIR_QUEUE_BUCKET_REASON,
        "agent_handoff": manual_repair_queue_agent_handoff(),
        "trust_boundary": MANUAL_REPAIR_QUEUE_ENTRY_TRUST_BOUNDARY,
    });
    if let Some(object) = value.as_object_mut() {
        if let Some(proof_mode) = &candidate.proof_mode {
            object.insert("proof_mode".to_string(), json!(proof_mode));
        }
        if let Some(fix_boundary) = &candidate.fix_boundary {
            object.insert("fix_boundary".to_string(), json!(fix_boundary));
        }
        if let Some(pr_aperture) = &candidate.pr_aperture {
            object.insert("pr_aperture".to_string(), json!(pr_aperture));
        }
        if let Some(stable_byte) = &candidate.stable_byte {
            object.insert("stable_byte".to_string(), json!(stable_byte));
        }
        if let Some(seed) = stable_byte_seed {
            object.insert(
                "stable_byte_seed".to_string(),
                review_kit_stable_byte_seed(seed, candidate),
            );
        }
        if candidate.oracle_map.is_none() {
            object.remove("oracle_map");
        }
    }
    value
}

fn manual_repair_queue_agent_handoff() -> serde_json::Value {
    json!({
        "state": "copy_ready",
        "automatic": false,
        "reasons": [
            "manual candidate includes file:line, safe caller route, invariant, evidence, fix/test/non-goal guidance, and stop condition",
            "candidate must stay manual/advisory and separate from ReviewCard repair-queue.json"
        ]
    })
}

fn tokmd_packet_inputs(
    stable_byte_seed_ledger: &StableByteSeedLedger,
    matched_stable_byte_seeds: usize,
    comment_plan_input: serde_json::Value,
) -> serde_json::Value {
    json!({
        "manual-candidates.json": {
            "included": true,
            "relationship": "primary manual/advisory candidate index projected into packets"
        },
        "manual-repair-queue.json": {
            "included": true,
            "relationship": "copy-only manual repair handoff fields are projected through packet manual_repair_queue_item"
        },
        "cards.json": {
            "included": false,
            "limitation": "ReviewCard packet export is outside this manual-candidate slice"
        },
        "witness-plan.md": {
            "included": false,
            "limitation": "Markdown witness-plan content is not converted to packet JSON in this slice"
        },
        "receipt-audit.md": {
            "included": false,
            "limitation": "Saved receipt audit data is not converted to packet JSON in this slice"
        },
        "repair-queue.json": {
            "included": false,
            "limitation": "ReviewCard repair queue stays separate from manual candidate packets"
        },
        "comment-plan.json": comment_plan_input,
        "stable-byte seed ledger": tokmd_stable_byte_seed_ledger_input(
            stable_byte_seed_ledger,
            matched_stable_byte_seeds,
        )
    })
}

fn tokmd_comment_plan_input(comment_plan: Option<&str>) -> serde_json::Value {
    let Some(comment_plan) = comment_plan else {
        return json!({
            "included": false,
            "limitation": "Comment-plan review budget data is not available to this standalone tokmd packet render"
        });
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(comment_plan) else {
        return json!({
            "included": false,
            "limitation": "Comment-plan review budget data was present but could not be parsed"
        });
    };
    let summary = value.get("summary").cloned().unwrap_or_else(|| json!({}));
    json!({
        "included": true,
        "relationship": "ReviewCard-only comment-plan review budget is projected for future bun-ub-review-map packets; manual candidates are not selected for automatic comment plans",
        "summary": {
            "selected_count": summary.get("selected_count").cloned().unwrap_or_else(|| json!(0)),
            "not_selected_count": summary.get("not_selected_count").cloned().unwrap_or_else(|| json!(0)),
            "budget": summary.get("budget").cloned().unwrap_or_else(|| json!(0)),
            "reason": summary.get("reason").cloned().unwrap_or_else(|| json!("")),
            "reason_code": summary.get("reason_code").cloned().unwrap_or_else(|| json!("")),
        },
        "selected_reason_codes": tokmd_comment_plan_reason_counts(
            value.get("comments"),
            "selection_reason_code",
        ),
        "not_selected_reason_codes": tokmd_comment_plan_reason_counts(
            value.get("not_selected"),
            "reason_code",
        ),
        "trust_boundary": "Plan-only ReviewCard comment budget metadata; unsafe-review did not post comments, did not import manual candidates into comment-plan.json, did not run witnesses, or make policy decisions."
    })
}

fn tokmd_comment_plan_reason_counts(
    entries: Option<&serde_json::Value>,
    field: &str,
) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    let Some(entries) = entries.and_then(serde_json::Value::as_array) else {
        return counts;
    };
    for entry in entries {
        if let Some(value) = entry.get(field).and_then(serde_json::Value::as_str) {
            *counts.entry(value.to_string()).or_insert(0) += 1;
        }
    }
    counts
}

fn tokmd_stable_byte_seed_ledger_input(
    stable_byte_seed_ledger: &StableByteSeedLedger,
    matched_stable_byte_seeds: usize,
) -> serde_json::Value {
    if !stable_byte_seed_ledger.present {
        return json!({
            "included": false,
            "limitation": "External seed ledger rows are not imported; packet-local stable_byte.ledger_state is preserved when the manual candidate supplies it"
        });
    }
    if let Some(parse_error) = &stable_byte_seed_ledger.parse_error {
        return json!({
            "included": false,
            "path": stable_byte_seed_ledger.path,
            "limitation": format!("Stable-byte seed ledger was present but not imported: {parse_error}; packet-local stable_byte.ledger_state is preserved when the manual candidate supplies it")
        });
    }
    json!({
        "included": true,
        "path": stable_byte_seed_ledger.path,
        "relationship": "root-local stable-byte seed ledger rows are joined to manual-candidate packets by the ID inside each row's referenced manual candidate JSON",
        "rows": stable_byte_seed_ledger.rows,
        "matched_manual_candidates": matched_stable_byte_seeds,
        "limitation": "Stable-byte seed rows are advisory workflow metadata only; not analyzer-discovered, not proof, not witness execution, not policy-ready, and not rendered tokmd output"
    })
}

fn tokmd_packet_entry(
    root: &Path,
    candidate: &ManualCandidate,
    stable_byte_seed: Option<&StableByteSeed>,
    stable_byte_seed_ledger: &StableByteSeedLedger,
) -> serde_json::Value {
    let ledger_state = stable_byte_ledger_state(candidate);
    let mut value = json!({
        "id": candidate.id.as_str(),
        "source": "manual",
        "manual_candidate": true,
        "analyzer_discovered": false,
        "packet_kind": "manual_candidate",
        "tokmd_presets": TOKMD_PACKET_PRESETS,
        "title": candidate.title.as_str(),
        "stable_byte_source_class": stable_byte_source_class(candidate),
        "stable_byte": candidate.stable_byte.as_ref(),
        "ledger_state": ledger_state,
        "ledger_state_limitation": tokmd_ledger_state_limitation(
            ledger_state.is_some(),
            stable_byte_seed.is_some(),
        ),
        "target": tokmd_manual_candidate_target(candidate),
        "route": {
            "safe_caller": candidate.safe_caller.as_str(),
            "unsafe_operation": candidate.unsafe_operation.as_str(),
            "operation_family": candidate.operation_family.as_str(),
        },
        "invariant_at_risk": candidate.invariant.as_str(),
        "external_evidence": candidate.evidence.iter().map(|evidence| json!({
            "kind": evidence.kind.as_str(),
            "path": evidence.path.as_ref().map(|path| path.display().to_string()),
            "summary": evidence.summary.as_deref(),
            "command": evidence.command.as_deref(),
            "limitation": evidence.limitation.as_deref(),
        })).collect::<Vec<_>>(),
        "oracle_map": candidate.oracle_map.as_ref(),
        "fix_options": &candidate.fix_options,
        "test_targets": &candidate.test_targets,
        "do_not_touch": &candidate.do_not_touch,
        "stable_byte_seed": stable_byte_seed.map(|seed| tokmd_stable_byte_seed(seed, candidate)),
        "implementer_handoff": manual_candidate_implementer_handoff(candidate),
        "manual_repair_queue_item": tokmd_manual_repair_queue_item(candidate),
        "preset_inputs": tokmd_preset_inputs(candidate, stable_byte_seed),
        "commands": {
            "explain": explain_command(root, &candidate.id),
            "context_json": context_command(root, &candidate.id),
            "witness_plan": candidate_witness_plan_command(root, &candidate.id),
        },
        "missing_inputs": tokmd_packet_missing_inputs(
            candidate,
            stable_byte_seed,
            stable_byte_seed_ledger,
        ),
        "trust_boundary": "Manual candidate tokmd packet input only; not analyzer-discovered, not tokmd output, not automatic repair, not witness execution, not source editing, not proof, and not policy gating.",
    });
    if let Some(object) = value.as_object_mut() {
        if let Some(proof_mode) = &candidate.proof_mode {
            object.insert("proof_mode".to_string(), json!(proof_mode));
        }
        if let Some(fix_boundary) = &candidate.fix_boundary {
            object.insert("fix_boundary".to_string(), json!(fix_boundary));
        }
        if let Some(pr_aperture) = &candidate.pr_aperture {
            object.insert("pr_aperture".to_string(), json!(pr_aperture));
        }
        if candidate.oracle_map.is_none() {
            object.remove("oracle_map");
        }
        if candidate.stable_byte.is_none() {
            object.remove("stable_byte");
        }
        if stable_byte_seed.is_none() {
            object.remove("stable_byte_seed");
        }
    }
    value
}

fn tokmd_ledger_state_limitation(
    has_packet_ledger_state: bool,
    has_stable_byte_seed: bool,
) -> &'static str {
    match (has_packet_ledger_state, has_stable_byte_seed) {
        (true, true) => {
            "packet-local manual candidate metadata is copied into ledger_state; stable_byte_seed projects the joined seed row separately as advisory workflow metadata"
        }
        (false, true) => {
            "packet-local ledger state is absent from manual candidate metadata; stable_byte_seed projects the joined seed row separately as advisory workflow metadata"
        }
        (true, false) => {
            "ledger state is packet-local manual candidate metadata; no joined stable-byte seed row is projected for this packet"
        }
        (false, false) => {
            "ledger state is not present in this manual candidate; use the stable-byte seed ledger or a future seed JSON export"
        }
    }
}

fn tokmd_preset_inputs(
    candidate: &ManualCandidate,
    stable_byte_seed: Option<&StableByteSeed>,
) -> serde_json::Value {
    json!({
        "bun-ub-handoff": tokmd_bun_ub_handoff_input(candidate, stable_byte_seed),
        "bun-ub-pr-body": tokmd_bun_ub_pr_body_input(candidate),
        "bun-ub-ledger-note": tokmd_bun_ub_ledger_note_input(candidate, stable_byte_seed),
        "bun-ub-review-map": tokmd_bun_ub_review_map_input(candidate),
        "bun-ub-next-pick": tokmd_bun_ub_next_pick_input(candidate, stable_byte_seed),
        "trust_boundary": "Preset inputs are copy-only formatting inputs for future tokmd rendering; unsafe-review did not run tokmd, post comments, execute witnesses, edit source, prove UB, prove memory safety, or make policy decisions."
    })
}

fn tokmd_bun_ub_handoff_input(
    candidate: &ManualCandidate,
    stable_byte_seed: Option<&StableByteSeed>,
) -> serde_json::Value {
    json!({
        "audience": "rust lane implementer",
        "candidate_id": candidate.id.as_str(),
        "title": candidate.title.as_str(),
        "stable_byte_family": stable_byte_source_class(candidate),
        "invariant_at_risk": candidate.invariant.as_str(),
        "safe_js_caller_route": candidate.safe_caller.as_str(),
        "rust_native_seam": tokmd_rust_native_seam(candidate),
        "target": tokmd_manual_candidate_target(candidate),
        "proof_mode": candidate.proof_mode.as_ref(),
        "required_proof_action": tokmd_required_proof_action(candidate),
        "current_evidence": tokmd_evidence_summaries(candidate),
        "fix_boundary": candidate.fix_boundary.as_deref(),
        "pr_aperture": candidate.pr_aperture.as_deref(),
        "test_or_witness_targets": &candidate.test_targets,
        "do_not_touch": &candidate.do_not_touch,
        "ledger_state": stable_byte_ledger_state(candidate),
        "seed": stable_byte_seed.map(tokmd_seed_summary),
        "next_action": tokmd_next_action(candidate, stable_byte_seed),
        "stop_line": tokmd_stop_line(candidate),
    })
}

fn tokmd_manual_candidate_target(candidate: &ManualCandidate) -> serde_json::Value {
    json!({
        "file": candidate.location.file.display().to_string(),
        "line": candidate.location.line,
        "location_text": manual_candidate_location_text(candidate),
    })
}

fn tokmd_bun_ub_pr_body_input(candidate: &ManualCandidate) -> serde_json::Value {
    json!({
        "audience": "upstream maintainer",
        "candidate_id": candidate.id.as_str(),
        "problem_statement": candidate.title.as_str(),
        "risk_statement": candidate.invariant.as_str(),
        "smallest_changed_surface": candidate.pr_aperture.as_deref().or(candidate.fix_boundary.as_deref()),
        "compatibility_oracle": candidate.oracle_map.as_ref(),
        "tests": &candidate.test_targets,
        "external_evidence": tokmd_evidence_summaries(candidate),
        "non_goals": &candidate.do_not_touch,
        "claims_not_made": tokmd_claims_not_made(),
    })
}

fn tokmd_bun_ub_ledger_note_input(
    candidate: &ManualCandidate,
    stable_byte_seed: Option<&StableByteSeed>,
) -> serde_json::Value {
    json!({
        "audience": "Bun burndown ledger maintainer",
        "candidate_id": candidate.id.as_str(),
        "current_ledger_state": stable_byte_ledger_state(candidate),
        "state_transition": "not requested by this packet export",
        "evidence_or_receipt": tokmd_evidence_summaries(candidate),
        "seed": stable_byte_seed.map(tokmd_seed_summary),
        "missing_transition_inputs": [
            "old/new ledger-state decision",
            "upstream PR, fork branch, receipt, or exact parked-followup unblock"
        ],
        "remaining_outside_aperture": &candidate.do_not_touch,
        "trust_boundary": "Ledger preset input only; ledger state is workflow metadata, not proof or policy readiness."
    })
}

fn tokmd_bun_ub_review_map_input(candidate: &ManualCandidate) -> serde_json::Value {
    json!({
        "audience": "reviewer deciding what to inspect first",
        "candidate_id": candidate.id.as_str(),
        "candidate_ids": [candidate.id.as_str()],
        "changed_files_or_seams": [
            manual_candidate_location_text(candidate),
            tokmd_rust_native_seam(candidate),
        ],
        "safe_js_caller_route": candidate.safe_caller.as_str(),
        "oracle_map": candidate.oracle_map.as_ref(),
        "comment_plan": {
            "source": "bundle inputs.comment-plan.json",
            "relationship": "ReviewCard-only review budget metadata; manual candidates are not selected for automatic comments"
        },
        "repair_queue": tokmd_manual_repair_queue_item(candidate),
        "explicit_no_posting_boundary": "unsafe-review did not post comments and this preset input does not authorize posting"
    })
}

fn tokmd_bun_ub_next_pick_input(
    candidate: &ManualCandidate,
    stable_byte_seed: Option<&StableByteSeed>,
) -> serde_json::Value {
    json!({
        "audience": "lane coordinator",
        "candidate_id": candidate.id.as_str(),
        "owner_lane": stable_byte_seed.map(|seed| seed.owner_lane.as_str()),
        "proof_mode": candidate.proof_mode.as_ref(),
        "required_proof_action": tokmd_required_proof_action(candidate),
        "smallest_first_pr": stable_byte_seed
            .map(|seed| seed.suggested_first_pr.as_str())
            .or(candidate.pr_aperture.as_deref())
            .or(candidate.fix_boundary.as_deref()),
        "dependencies_or_unblock": tokmd_dependencies_or_unblock(candidate),
        "non_goals": &candidate.do_not_touch,
        "next_action": tokmd_next_action(candidate, stable_byte_seed),
        "trust_boundary": "Next-pick preset input is routing metadata only; it does not rank by calibrated recall or claim proof."
    })
}

fn tokmd_seed_summary(seed: &StableByteSeed) -> serde_json::Value {
    json!({
        "seed_id": seed.seed_id.as_str(),
        "ledger_state": seed.ledger_state.as_str(),
        "owner_lane": seed.owner_lane.as_str(),
        "suggested_first_pr": seed.suggested_first_pr.as_str(),
        "triage_labels": &seed.triage_labels,
    })
}

fn tokmd_evidence_summaries(candidate: &ManualCandidate) -> Vec<serde_json::Value> {
    candidate
        .evidence
        .iter()
        .map(|evidence| {
            json!({
                "kind": evidence.kind.as_str(),
                "path": evidence.path.as_ref().map(|path| path.display().to_string()),
                "summary": evidence.summary.as_deref(),
                "command": evidence.command.as_deref(),
                "limitation": evidence.limitation.as_deref(),
            })
        })
        .collect()
}

fn tokmd_rust_native_seam(candidate: &ManualCandidate) -> String {
    candidate
        .stable_byte
        .as_ref()
        .map(|stable_byte| stable_byte.sink.clone())
        .unwrap_or_else(|| manual_candidate_location_text(candidate))
}

fn tokmd_required_proof_action(candidate: &ManualCandidate) -> &'static str {
    match candidate
        .proof_mode
        .as_ref()
        .map(|proof_mode| proof_mode.kind.as_str())
    {
        Some("observable-red-green") => {
            "collect system-Bun red evidence and patched-green evidence for the smallest PR aperture"
        }
        Some("mutation-plus-miri") => {
            "pair mutation pressure with a focused Miri/model proof of the byte-lifetime shape"
        }
        Some("source-route-only") => {
            "preserve route evidence and do not call the candidate sure UB from source inspection alone"
        }
        Some("helper-gated") => {
            "park as a verified follow-up until exact helper semantics or unblock command is recorded"
        }
        _ => "record an explicit proof mode before changing ledger state or implementation claims",
    }
}

fn tokmd_dependencies_or_unblock(candidate: &ManualCandidate) -> Vec<&'static str> {
    match candidate
        .proof_mode
        .as_ref()
        .map(|proof_mode| proof_mode.kind.as_str())
    {
        Some("helper-gated") => vec!["exact helper semantics or unblock command"],
        Some("mutation-plus-miri") => vec!["mutation pressure", "focused Miri/model proof"],
        Some("observable-red-green") => vec!["system-Bun red", "patched-green"],
        Some("source-route-only") => vec!["stronger proof before sure-UB wording"],
        _ => vec!["proof mode"],
    }
}

fn tokmd_next_action(
    candidate: &ManualCandidate,
    stable_byte_seed: Option<&StableByteSeed>,
) -> String {
    if let Some(seed) = stable_byte_seed {
        return format!(
            "start `{}` in `{}`; {}",
            seed.suggested_first_pr,
            seed.owner_lane,
            tokmd_required_proof_action(candidate)
        );
    }
    format!(
        "use `{}` and {}; keep the change inside the candidate PR aperture",
        candidate.id,
        tokmd_required_proof_action(candidate)
    )
}

fn tokmd_stop_line(candidate: &ManualCandidate) -> String {
    candidate
        .pr_aperture
        .as_ref()
        .map(|aperture| format!("stop at PR aperture: {aperture}"))
        .unwrap_or_else(|| "stop before broadening into unrelated unsafe sites".to_string())
}

fn tokmd_claims_not_made() -> Vec<&'static str> {
    vec![
        "not proof of UB",
        "not proof of memory safety",
        "not UB-free status",
        "not Miri-clean status",
        "not site-execution proof",
        "not calibrated precision or recall",
        "not policy readiness",
        "not automatic repair",
    ]
}

fn tokmd_stable_byte_seed(seed: &StableByteSeed, candidate: &ManualCandidate) -> serde_json::Value {
    json!({
        "source": STABLE_BYTE_SEED_LEDGER_PATH,
        "seed_id": seed.seed_id.as_str(),
        "ledger_state": seed.ledger_state.as_str(),
        "candidate_family": seed.candidate_family.as_str(),
        "surface": seed.surface.as_str(),
        "manual_candidate": seed.manual_candidate.as_str(),
        "safe_js_caller": seed.safe_js_caller.as_str(),
        "rust_native_sink": seed.rust_native_sink.as_str(),
        "proof_mode": seed.proof_mode.as_str(),
        "suggested_first_pr": seed.suggested_first_pr.as_str(),
        "manual_candidate_pr_aperture": candidate.pr_aperture.as_deref(),
        "owner_lane": seed.owner_lane.as_str(),
        "triage_labels": &seed.triage_labels,
        "candidate_consistency": {
            "stable_byte_class_matches_manual_candidate": stable_byte_source_class(candidate)
                == Some(seed.candidate_family.as_str()),
            "proof_mode_matches_manual_candidate": candidate.proof_mode.as_ref()
                .map(|proof_mode| proof_mode.kind.as_str())
                == Some(seed.proof_mode.as_str()),
            "ledger_state_matches_manual_candidate": stable_byte_ledger_state(candidate)
                == Some(seed.ledger_state.as_str()),
            "safe_js_caller_matches_manual_candidate": stable_byte_source(candidate)
                == Some(seed.safe_js_caller.as_str()),
            "rust_native_sink_matches_manual_candidate": stable_byte_sink(candidate)
                == Some(seed.rust_native_sink.as_str()),
            "suggested_first_pr_has_manual_candidate_pr_aperture": !seed.suggested_first_pr.trim().is_empty()
                && candidate.pr_aperture.as_ref().is_some_and(|value| !value.trim().is_empty()),
        },
        "trust_boundary": "Stable-byte seed row is advisory workflow metadata only; not analyzer discovery, not witness execution, not proof, not policy readiness, and not rendered tokmd output."
    })
}

fn tokmd_manual_repair_queue_item(candidate: &ManualCandidate) -> serde_json::Value {
    json!({
        "artifact": "manual-repair-queue.json",
        "id": candidate.id.as_str(),
        "bucket": MANUAL_REPAIR_QUEUE_BUCKET,
        "bucket_reason": MANUAL_REPAIR_QUEUE_BUCKET_REASON,
        "agent_handoff": manual_repair_queue_agent_handoff(),
        "trust_boundary": MANUAL_REPAIR_QUEUE_ENTRY_TRUST_BOUNDARY,
    })
}

fn tokmd_packet_missing_inputs(
    candidate: &ManualCandidate,
    stable_byte_seed: Option<&StableByteSeed>,
    stable_byte_seed_ledger: &StableByteSeedLedger,
) -> Vec<&'static str> {
    let mut missing = vec!["ReviewCard projection", "receipt audit JSON"];
    if stable_byte_ledger_state(candidate).is_none() {
        missing.push("stable-byte ledger state");
    }
    if stable_byte_seed_ledger.present
        && stable_byte_seed_ledger.parse_error.is_none()
        && stable_byte_seed.is_none()
    {
        missing.push("stable-byte seed row");
    }
    missing
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

fn manual_candidate_reviewcard_applicability() -> serde_json::Value {
    json!({
        "cards.json": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates stay in manual-candidate ledger surfaces and are not emitted as analyzer ReviewCards."
        ),
        "cards.sarif": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not emitted as SARIF analyzer results."
        ),
        "comment-plan.json": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not selected for automatic comment plans."
        ),
        "lsp.json": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not emitted as saved editor diagnostics."
        ),
        "repair-queue.json": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not automatic repair tasks."
        ),
        "policy-report.json": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not policy gating inputs for the JSON policy report."
        ),
        "policy-report.md": manual_candidate_reviewcard_applicability_entry(
            "reviewcard_only",
            "Manual candidates are not policy gating inputs for the Markdown policy report."
        )
    })
}

fn manual_candidate_reviewcard_applicability_entry(
    decision: &str,
    reason: &str,
) -> serde_json::Value {
    json!({
        "decision": decision,
        "applies_to_manual_candidates": false,
        "manual_candidate_markers_allowed": false,
        "reason": reason,
    })
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
        "unsafe-review-gate.json" => "gate_manifest",
        "cards.json" => "review_cards",
        "pr-summary.md" => "reviewer_summary",
        "github-summary.md" => "github_summary",
        "cards.sarif" => "sarif",
        "comment-plan.json" => "comment_plan",
        "witness-plan.md" => "witness_plan",
        "receipt-audit.md" => "receipt_audit",
        "receipt-audit.json" => "receipt_audit",
        "policy-report.json" => "policy_report_json",
        "policy-report.md" => "policy_report_markdown",
        "manual-candidates.json" => "manual_candidates",
        "manual-repair-queue.json" => "manual_repair_queue",
        "tokmd-packets.json" => "tokmd_packets",
        "lsp.json" => "saved_lsp",
        "repair-queue.json" => "repair_queue",
        "usefulness-telemetry.json" => "usefulness_telemetry",
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
        // cards.json was bumped to 0.2 when provenance metadata was added.
        "cards.json" => Some("0.2"),
        "review-kit.json" | "comment-plan.json" | "lsp.json" | "repair-queue.json"
        | "policy-report.json" | "receipt-audit.json" => Some("0.1"),
        "unsafe-review-gate.json" => Some("unsafe-review-gate/v1"),
        "manual-candidates.json" => Some("manual-candidates/v1"),
        "manual-repair-queue.json" => Some("manual-repair-queue/v1"),
        "tokmd-packets.json" => Some("tokmd-packets/v1"),
        "cards.sarif" => Some("2.1.0"),
        "usefulness-telemetry.json" => Some("usefulness-telemetry/v1"),
        _ => None,
    }
}

fn print_artifact_paths(out_dir: &Path, artifacts: &[&str]) {
    println!("Artifacts:");
    for name in artifacts {
        println!("  {}", artifact_path_display(out_dir, name));
    }
}

fn print_trust_boundary() {
    println!("Trust boundary:");
    println!(
        "  static unsafe contract review only; not memory-safety proof, not UB-free status, not Miri-clean status, and not a site-execution claim unless a matching witness receipt says so."
    );
    println!(
        "  unsafe-review did not run witnesses, post comments, edit source, or enforce blocking policy."
    );
}

fn card_path_display(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// Return a forward-slash-normalised display string for an artifact path,
/// joining `base` and `name` exactly as the file is written on disk but
/// normalising the separator so every `println!` surface shows `/` on all
/// platforms.  On-disk paths and machine-readable outputs are unaffected.
fn artifact_path_display(base: &Path, name: &str) -> String {
    base.join(name).to_string_lossy().replace('\\', "/")
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
    fn usefulness_telemetry_artifact_is_classified_not_unknown() {
        // Regression: usefulness-telemetry.json (SPEC-0038) must be a known
        // review-kit artifact, or check-first-pr-artifacts rejects the bundle
        // with an unknown kind. Producer kind/format/schema must match the
        // xtask expectation (advisory_artifacts::expected_review_kit_*).
        assert_eq!(
            artifact_kind("usefulness-telemetry.json"),
            "usefulness_telemetry"
        );
        assert_ne!(artifact_kind("usefulness-telemetry.json"), "unknown");
        assert_eq!(artifact_format("usefulness-telemetry.json"), "json");
        assert_eq!(
            artifact_schema_version("usefulness-telemetry.json"),
            Some("usefulness-telemetry/v1")
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
            diff_scoped_files: std::collections::BTreeSet::new(),
            coverage_snapshot: std::collections::BTreeMap::new(),
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
        assert_eq!(
            value["handoff"]["manual_candidates"]["reviewcard_artifact_applicability"]["policy-report.json"]
                ["decision"],
            "reviewcard_only"
        );
        assert_eq!(
            value["handoff"]["manual_candidates"]["reviewcard_artifact_applicability"]["policy-report.md"]
                ["decision"],
            "reviewcard_only"
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
        assert_eq!(value["artifacts"][1]["schema_version"], "0.2");
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
                .contains("- Evidence refs: 1; full route and evidence packet in sidecars.")
        );
        assert!(!github_summary.contains("- Safe caller route:"));
        assert!(!github_summary.contains("- Invariant at risk:"));
        assert!(github_summary.contains("- Proof mode: `mutation-plus-miri`"));
        assert!(github_summary.contains(
            "- Fix boundary: Snapshot SharedArrayBuffer-backed bytes before constructing the slice"
        ));
        assert!(github_summary.contains("- PR aperture: TextDecoder shared-byte snapshot only"));
        assert!(github_summary.contains("- Stop line: keep the PR inside this aperture."));
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
        assert!(!github_summary.contains("- Manual candidate queue preview:"));
        assert!(
            !github_summary
                .contains("unsafe-review explain --root \"fixtures/bun fork\" R4R2-S001")
        );
        assert!(
            github_summary
                .contains("unsafe-review context --root \"fixtures/bun fork\" R4R2-S001 --json")
        );
        assert!(github_summary.contains(
            "unsafe-review candidate witness-plan --root \"fixtures/bun fork\" R4R2-S001"
        ));
        assert!(github_summary.contains("ReviewCard-only outputs clean"));
        assert!(github_summary.contains("manual-repair-queue.json"));
        assert!(github_summary.contains("separate from ReviewCard `repair-queue.json`"));
        assert!(github_summary.contains("no agent was run"));
        assert!(github_summary.contains("did not discover"));
        assert!(github_summary.contains("did not run witnesses"));
        assert!(github_summary.contains("edit source"));
        assert!(github_summary.contains("policy inputs"));
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
        assert!(pr_summary.contains(
            "- First fix option: Copy SharedArrayBuffer-backed bytes before constructing the slice"
        ));
        assert!(pr_summary.contains(
            "- First test target: `test/js/webcore/textdecoder-sharedarraybuffer.test.ts`"
        ));
        assert!(
            pr_summary
                .contains("- First do-not-touch note: Do not rewrite unrelated TextDecoder paths")
        );
        assert!(
            pr_summary.contains("unsafe-review explain --root \"fixtures/bun fork\" R4R2-S001")
        );
        assert!(pr_summary.contains("candidates stay out of ReviewCard-only outputs"));
        assert!(pr_summary.contains(
            "Manual repair queue: `manual-repair-queue.json`; copy-only manual candidate repair handoff, separate from ReviewCard `repair-queue.json`; no agent was run"
        ));
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

    #[test]
    fn tokmd_packets_join_root_local_stable_byte_seed_rows() -> Result<(), String> {
        let root = unique_test_root("unsafe-review-tokmd-seed-ledger")?;
        let docs_dir = root.join("docs/dogfood");
        fs::create_dir_all(&docs_dir)
            .map_err(|err| format!("create {} failed: {err}", docs_dir.display()))?;
        let candidate_dir = root.join("docs/examples/manual-candidates");
        fs::create_dir_all(&candidate_dir)
            .map_err(|err| format!("create {} failed: {err}", candidate_dir.display()))?;
        let candidate = manual_candidate_fixture_with_stable_byte()?;
        fs::write(
            candidate_dir.join("textdecoder-sab.json"),
            manual_candidate_fixture_with_stable_byte_json(),
        )
        .map_err(|err| format!("write candidate fixture failed: {err}"))?;
        fs::write(
            docs_dir.join("stable-byte-follow-up-seeds.md"),
            r#"# Bun stable-byte follow-up seed index

## Seeds

| Seed ID | Ledger state | Candidate family | Surface | Manual candidate | Safe JS caller | Rust/native sink | Proof mode | Suggested first PR | Owner lane | Triage labels |
|---|---|---|---|---|---|---|---|---|---|---|
| `bun-stable-byte-textdecoder-sab` | `handoff-ready` | `stable-byte-source-sab-race` | `TextDecoder.decode` | `docs/examples/manual-candidates/textdecoder-sab.json` | SharedArrayBuffer-backed typed array decode | `src/runtime/webcore/TextDecoder.rs` slice materialization | `mutation-plus-miri` | `TextDecoder shared-byte snapshot only` | `rust2` | `non-observable`, `needs-miri-model` |
"#,
        )
        .map_err(|err| format!("write seed ledger failed: {err}"))?;

        let rendered = render_tokmd_packets_artifact(&root, std::slice::from_ref(&candidate), None);
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("tokmd packets should render JSON: {err}"))?;

        assert_eq!(value["summary"]["with_stable_byte_seed"], 1);
        assert_eq!(value["inputs"]["comment-plan.json"]["included"], false);
        assert!(
            value["inputs"]["comment-plan.json"]["limitation"]
                .as_str()
                .unwrap_or("")
                .contains("standalone tokmd packet render")
        );
        assert_eq!(value["inputs"]["stable-byte seed ledger"]["included"], true);
        assert_eq!(
            value["inputs"]["stable-byte seed ledger"]["matched_manual_candidates"],
            1
        );
        let seed = &value["packets"][0]["stable_byte_seed"];
        assert_eq!(seed["seed_id"], "bun-stable-byte-textdecoder-sab");
        assert_eq!(seed["owner_lane"], "rust2");
        assert_eq!(
            seed["suggested_first_pr"],
            "TextDecoder shared-byte snapshot only"
        );
        assert_eq!(
            seed["manual_candidate_pr_aperture"],
            "TextDecoder shared-byte snapshot only; do not rewrite unrelated encodings"
        );
        assert_eq!(seed["triage_labels"][1], "needs-miri-model");
        assert_eq!(
            seed["candidate_consistency"]["stable_byte_class_matches_manual_candidate"],
            true
        );
        assert_eq!(
            seed["candidate_consistency"]["proof_mode_matches_manual_candidate"],
            true
        );
        assert_eq!(
            seed["candidate_consistency"]["ledger_state_matches_manual_candidate"],
            true
        );
        assert_eq!(
            seed["candidate_consistency"]["safe_js_caller_matches_manual_candidate"],
            true
        );
        assert_eq!(
            seed["candidate_consistency"]["rust_native_sink_matches_manual_candidate"],
            true
        );
        assert_eq!(
            seed["candidate_consistency"]["suggested_first_pr_has_manual_candidate_pr_aperture"],
            true
        );
        let handoff_target = &value["packets"][0]["preset_inputs"]["bun-ub-handoff"]["target"];
        assert_eq!(handoff_target["file"], "src/runtime/webcore/TextDecoder.rs");
        assert_eq!(handoff_target["line"], 237);
        assert_eq!(
            handoff_target["location_text"],
            "src/runtime/webcore/TextDecoder.rs:237"
        );
        let ledger_limitation = value["packets"][0]["ledger_state_limitation"]
            .as_str()
            .unwrap_or("");
        assert!(ledger_limitation.contains("packet-local manual candidate metadata"));
        assert!(ledger_limitation.contains("joined seed row"));
        assert!(!ledger_limitation.contains("external seed ledger rows are not imported"));
        assert!(
            seed["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not rendered tokmd output")
        );
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn manual_repair_queue_joins_root_local_stable_byte_seed_rows() -> Result<(), String> {
        let root = unique_test_root("unsafe-review-manual-repair-seed-ledger")?;
        let docs_dir = root.join("docs/dogfood");
        fs::create_dir_all(&docs_dir)
            .map_err(|err| format!("create {} failed: {err}", docs_dir.display()))?;
        let candidate_dir = root.join("docs/examples/manual-candidates");
        fs::create_dir_all(&candidate_dir)
            .map_err(|err| format!("create {} failed: {err}", candidate_dir.display()))?;
        let candidate = manual_candidate_fixture_with_stable_byte()?;
        fs::write(
            candidate_dir.join("textdecoder-sab.json"),
            manual_candidate_fixture_with_stable_byte_json(),
        )
        .map_err(|err| format!("write candidate fixture failed: {err}"))?;
        fs::write(
            docs_dir.join("stable-byte-follow-up-seeds.md"),
            r#"# Bun stable-byte follow-up seed index

## Seeds

| Seed ID | Ledger state | Candidate family | Surface | Manual candidate | Safe JS caller | Rust/native sink | Proof mode | Suggested first PR | Owner lane | Triage labels |
|---|---|---|---|---|---|---|---|---|---|---|
| `bun-stable-byte-textdecoder-sab` | `handoff-ready` | `stable-byte-source-sab-race` | `TextDecoder.decode` | `docs/examples/manual-candidates/textdecoder-sab.json` | SharedArrayBuffer-backed typed array decode | `src/runtime/webcore/TextDecoder.rs` slice materialization | `mutation-plus-miri` | `TextDecoder shared-byte snapshot only` | `rust2` | `non-observable`, `needs-miri-model` |
"#,
        )
        .map_err(|err| format!("write seed ledger failed: {err}"))?;

        let rendered = render_manual_repair_queue_artifact(&root, std::slice::from_ref(&candidate));
        let value: serde_json::Value = serde_json::from_str(&rendered)
            .map_err(|err| format!("manual repair queue should render JSON: {err}"))?;

        assert_eq!(value["summary"]["with_stable_byte_seed"], 1);
        assert_eq!(
            value["summary"]["stable_byte_seed_source"]["included"],
            true
        );
        assert_eq!(
            value["summary"]["stable_byte_seed_source"]["matched_manual_candidates"],
            1
        );
        assert!(
            value["summary"]["stable_byte_seed_source"]["relationship"]
                .as_str()
                .unwrap_or("")
                .contains("manual-repair-queue entries")
        );
        let seed = &value["queue"][0]["stable_byte_seed"];
        assert_eq!(seed["seed_id"], "bun-stable-byte-textdecoder-sab");
        assert_eq!(seed["owner_lane"], "rust2");
        assert_eq!(
            seed["suggested_first_pr"],
            "TextDecoder shared-byte snapshot only"
        );
        assert_eq!(
            seed["manual_candidate_pr_aperture"],
            "TextDecoder shared-byte snapshot only; do not rewrite unrelated encodings"
        );
        assert_eq!(seed["triage_labels"][1], "needs-miri-model");
        assert_eq!(
            seed["candidate_consistency"]["stable_byte_class_matches_manual_candidate"],
            true
        );
        assert_eq!(
            seed["candidate_consistency"]["proof_mode_matches_manual_candidate"],
            true
        );
        assert_eq!(
            seed["candidate_consistency"]["ledger_state_matches_manual_candidate"],
            true
        );
        assert_eq!(
            seed["candidate_consistency"]["safe_js_caller_matches_manual_candidate"],
            true
        );
        assert_eq!(
            seed["candidate_consistency"]["rust_native_sink_matches_manual_candidate"],
            true
        );
        assert_eq!(
            seed["candidate_consistency"]["suggested_first_pr_has_manual_candidate_pr_aperture"],
            true
        );
        assert!(
            seed["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not a ReviewCard truth")
        );
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn stable_byte_seed_ledger_rejects_duplicate_manual_candidate_ids() -> Result<(), String> {
        let root = unique_test_root("unsafe-review-duplicate-seed-ledger")?;
        let candidate_dir = root.join("docs/examples/manual-candidates");
        fs::create_dir_all(&candidate_dir)
            .map_err(|err| format!("create {} failed: {err}", candidate_dir.display()))?;
        fs::write(
            candidate_dir.join("textdecoder-sab.json"),
            manual_candidate_fixture_with_stable_byte_json(),
        )
        .map_err(|err| format!("write candidate fixture failed: {err}"))?;
        let text = r#"# Bun stable-byte follow-up seed index

## Seeds

| Seed ID | Ledger state | Candidate family | Surface | Manual candidate | Safe JS caller | Rust/native sink | Proof mode | Suggested first PR | Owner lane | Triage labels |
|---|---|---|---|---|---|---|---|---|---|---|
| `bun-stable-byte-textdecoder-sab` | `handoff-ready` | `stable-byte-source-sab-race` | `TextDecoder.decode` | `docs/examples/manual-candidates/textdecoder-sab.json` | SharedArrayBuffer-backed typed array decode | `src/runtime/webcore/TextDecoder.rs` slice materialization | `mutation-plus-miri` | `TextDecoder shared-byte snapshot only` | `rust2` | `non-observable`, `needs-miri-model` |
| `bun-stable-byte-textdecoder-sab-follow-up` | `handoff-ready` | `stable-byte-source-sab-race` | `TextDecoder.decode` | `docs/examples/manual-candidates/textdecoder-sab.json` | SharedArrayBuffer-backed typed array decode | `src/runtime/webcore/TextDecoder.rs` slice materialization | `mutation-plus-miri` | `TextDecoder shared-byte snapshot only` | `rust2` | `non-observable`, `needs-miri-model` |
"#;

        let err = match parse_stable_byte_seed_ledger(&root, text) {
            Ok(_) => return Err("duplicate stable-byte seed rows should fail".to_string()),
            Err(err) => err,
        };

        assert!(err.contains("both resolve to manual candidate `R4R2-S001`"));
        assert!(err.contains("at most one stable-byte seed row"));
        let _ = fs::remove_dir_all(&root);
        Ok(())
    }

    #[test]
    fn stable_byte_seed_ledger_rejects_duplicate_seed_ids() -> Result<(), String> {
        let root = unique_test_root("unsafe-review-duplicate-seed-id-ledger")?;
        let text = r#"# Bun stable-byte follow-up seed index

## Seeds

| Seed ID | Ledger state | Candidate family | Surface | Manual candidate | Safe JS caller | Rust/native sink | Proof mode | Suggested first PR | Owner lane | Triage labels |
|---|---|---|---|---|---|---|---|---|---|---|
| `bun-stable-byte-textdecoder-sab` | `handoff-ready` | `stable-byte-source-sab-race` | `TextDecoder.decode` | `docs/examples/manual-candidates/textdecoder-sab.json` | SharedArrayBuffer-backed typed array decode | `src/runtime/webcore/TextDecoder.rs` slice materialization | `mutation-plus-miri` | `TextDecoder shared-byte snapshot only` | `rust2` | `non-observable`, `needs-miri-model` |
| `bun-stable-byte-textdecoder-sab` | `handoff-ready` | `stable-byte-source-rab-async` | `node:fs write` | `docs/examples/manual-candidates/node-fs-rab-scalar-write.json` | RAB-backed BufferSource resized before async filesystem completion | `src/runtime/node/node_fs.rs` worker-side write input read | `observable-red-green` | `node:fs scalar write input snapshot only` | `rust3` | `observable` |
"#;

        let err = match parse_stable_byte_seed_ledger(&root, text) {
            Ok(_) => return Err("duplicate stable-byte seed ids should fail".to_string()),
            Err(err) => err,
        };

        assert!(err.contains("repeats seed id first seen on line"));
        assert!(err.contains("unique seed id"));
        let _ = fs::remove_dir_all(&root);
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
              "proof_mode": {
                "kind": "mutation-plus-miri",
                "system_bun_expected": "nondiscriminating",
                "mutation_required": true,
                "miri_required": true
              },
              "fix_boundary": "Snapshot SharedArrayBuffer-backed bytes before constructing the slice",
              "pr_aperture": "TextDecoder shared-byte snapshot only",
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
              "trust_boundary": "manual candidate; not analyzer-discovered; not witness execution; not proof of memory safety; not UB-free status; not Miri-clean status; not site-execution proof; not policy readiness"
            }"#,
        )
    }

    fn manual_candidate_fixture_with_stable_byte() -> Result<ManualCandidate, String> {
        ManualCandidate::from_json_str(manual_candidate_fixture_with_stable_byte_json())
    }

    fn manual_candidate_fixture_with_stable_byte_json() -> &'static str {
        r#"{
          "schema_version": "manual-candidate/v1",
          "id": "R4R2-S001",
          "source": "manual",
          "manual_candidate": true,
          "analyzer_discovered": false,
          "title": "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes",
          "location": {
            "file": "src/runtime/webcore/TextDecoder.rs",
            "line": 237
          },
          "operation_family": "raw_pointer_read",
          "unsafe_operation": "core::slice::from_raw_parts",
          "invariant": "&[u8] memory must not be concurrently mutated",
          "safe_caller": "TextDecoder.decode SharedArrayBuffer route",
          "stable_byte": {
            "class": "stable-byte-source-sab-race",
            "source": "SharedArrayBuffer-backed typed array decode",
            "sink": "src/runtime/webcore/TextDecoder.rs slice materialization",
            "hazard": "Rust slice materialization can treat shared JS bytes as stable while JS can mutate the backing storage concurrently",
            "observable": "no",
            "proof_required": "mutation-plus-miri",
            "suggested_fix_boundary": "copy shared bytes before constructing the Rust slice",
            "pr_aperture": "TextDecoder shared-byte snapshot only; do not rewrite unrelated encodings",
            "ledger_state": "handoff-ready"
          },
          "proof_mode": {
            "kind": "mutation-plus-miri",
            "system_bun_expected": "nondiscriminating",
            "mutation_required": true,
            "miri_required": true
          },
          "fix_boundary": "copy shared bytes before constructing the Rust slice",
          "pr_aperture": "TextDecoder shared-byte snapshot only; do not rewrite unrelated encodings",
          "evidence": [{
            "kind": "runtime_witness",
            "path": "target/unsafe-scout/textdecoder-shared-race-route.out",
            "summary": "Bun TextDecoder route reaches shared backing bytes through safe JS",
            "command": "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts",
            "limitation": "runtime route evidence only; not memory-safety proof and not analyzer-discovered"
          }],
          "trust_boundary": "manual candidate; not analyzer-discovered; not witness execution; not proof of memory safety; not UB-free status; not Miri-clean status; not site-execution proof; not policy readiness"
        }"#
    }

    #[test]
    fn artifact_path_display_normalises_separators() {
        // On every platform the helper must produce forward slashes only.
        // The base path is constructed with `Path::new` using a backslash-
        // containing string so that the test exercises the replacement on
        // platforms where `Path` does not convert separators itself.
        let base = Path::new("C:\\Users\\smoke\\out");
        assert_eq!(
            artifact_path_display(base, "pr-summary.md"),
            "C:/Users/smoke/out/pr-summary.md"
        );
        assert_eq!(
            artifact_path_display(base, "repair-queue.json"),
            "C:/Users/smoke/out/repair-queue.json"
        );
        assert_eq!(
            artifact_path_display(base, "policy-report.md"),
            "C:/Users/smoke/out/policy-report.md"
        );
        assert_eq!(
            artifact_path_display(base, "manual-candidates.json"),
            "C:/Users/smoke/out/manual-candidates.json"
        );
        assert_eq!(
            artifact_path_display(base, "manual-repair-queue.json"),
            "C:/Users/smoke/out/manual-repair-queue.json"
        );
        assert_eq!(
            artifact_path_display(base, "tokmd-packets.json"),
            "C:/Users/smoke/out/tokmd-packets.json"
        );
        // Verify no backslash is present in any result.
        for name in &[
            "pr-summary.md",
            "repair-queue.json",
            "policy-report.md",
            "manual-candidates.json",
            "manual-repair-queue.json",
            "tokmd-packets.json",
        ] {
            let display = artifact_path_display(base, name);
            assert!(
                !display.contains('\\'),
                "artifact_path_display({name}) produced backslash: {display}"
            );
        }
    }

    fn unique_test_root(name: &str) -> Result<std::path::PathBuf, String> {
        let suffix = format!(
            "{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|err| format!("system clock before epoch: {err}"))?
                .as_nanos()
        );
        let root = std::env::temp_dir().join(format!("{name}-{suffix}"));
        fs::create_dir_all(&root)
            .map_err(|err| format!("create {} failed: {err}", root.display()))?;
        Ok(root)
    }
}
