use crate::api::AnalyzeOutput;
use crate::api::Scope;
use crate::domain::{OperationFamily, ReviewCard};
use crate::output::confirmation::{
    build_this_first, confirmation_step, hypothesis_to_confirm, minimal_repro,
};
use crate::output::{
    NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE, REVIEWCARD_TRUST_BOUNDARY, UNKNOWN_OWNER,
};
use crate::output::{agent, repair_queue};
use crate::util::path_display;
use std::collections::{BTreeMap, BTreeSet};

const DEFAULT_REVIEW_ROUTE: &str = "human-deep-review";
const REPAIR_QUEUE_BUCKET_ORDER: [&str; 6] = [
    "repairable_by_guard",
    "repairable_by_safety_docs",
    "repairable_by_test",
    "requires_witness_receipt",
    "requires_human_review",
    "do_not_auto_repair",
];

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    if matches!(output.scope, Scope::Repo) {
        return render_repo_posture(output);
    }
    let mut out = String::new();
    out.push_str("# unsafe-review\n\n");
    out.push_str(&format!(
        "{} changed/repo unsafe seam card(s) found.\n\n",
        output.summary.cards
    ));
    out.push_str("## Recommended next action\n\n");
    if let Some(card) = output.cards.first() {
        out.push_str(&card.next_action.summary);
        out.push_str("\n\n");
        if let Some(cmd) = card.next_action.verify_commands.first() {
            push_bash_block(&mut out, cmd);
            out.push('\n');
        }
    } else {
        render_no_changed_gaps(&mut out);
    }
    out.push_str("## Cards\n\n");
    out.push_str(
        "| ID | Class | Proof path | Operation | Hazard | Missing | Route | Next action |\n",
    );
    out.push_str("|---|---|---|---|---|---|---|---|\n");
    for card in &output.cards {
        let hazard = card.hazards.first().map_or("unknown", |h| h.as_str());
        let missing = card.missing.first().map_or("", |m| m.kind.as_str());
        let route = diff_primary_route(card);
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} |\n",
            md_cell(&card.id.to_string()),
            card.class.as_str(),
            card.proof_path.as_str(),
            md_cell(&one_line(&card.operation.expression)),
            hazard,
            missing,
            route,
            md_cell(&card.next_action.summary)
        ));
    }
    out.push_str("\n## Trust boundary\n\n");
    push_reviewcard_trust_boundary(&mut out);
    out
}

fn render_repo_posture(output: &AnalyzeOutput) -> String {
    let mut out = String::new();
    out.push_str("# unsafe-review repo posture\n\n");
    out.push_str("Static repo-scope unsafe-review evidence projected from ReviewCards.\n\n");
    out.push_str("## Summary\n\n");
    out.push_str("| Cards | Open gaps | Contract missing | Guard missing | Guarded unwitnessed | Requires Loom | Miri unsupported | Static unknown |\n");
    out.push_str("|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    out.push_str(&format!(
        "| {} | {} | {} | {} | {} | {} | {} | {} |\n\n",
        output.summary.cards,
        output.summary.open_actionable_gaps,
        output.summary.contract_missing,
        output.summary.guard_missing,
        output.summary.guarded_unwitnessed,
        output.summary.requires_loom,
        output.summary.miri_unsupported,
        output.summary.static_unknown
    ));
    if output.summary.open_actionable_gaps == 0 {
        render_no_changed_gaps(&mut out);
    }

    out.push_str("## Top classes\n\n");
    render_counts_table(&mut out, "Class", class_counts(output));

    out.push_str("## Top operation families\n\n");
    render_counts_table(&mut out, "Operation family", operation_counts(output));

    out.push_str("## Witness routes\n\n");
    render_counts_table(&mut out, "Route", route_counts(output));

    render_related_sink_clusters(&mut out, output);

    out.push_str("## Cards\n\n");
    if output.cards.is_empty() {
        out.push_str("No repo-scope unsafe-review cards found.\n\n");
    } else {
        out.push_str(
            "| ID | Class | Proof path | Location | Operation family | Operation | Missing evidence | Route | Next action |\n",
        );
        out.push_str("|---|---|---|---|---|---|---|---|---|\n");
        for card in &output.cards {
            let route = repo_primary_route(card);
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | `{}` | `{}` | {} | `{}` | {} |\n",
                md_cell(&card.id.to_string()),
                card.class.as_str(),
                card.proof_path.as_str(),
                md_cell(&card_location(card)),
                card.operation.family.as_str(),
                md_cell(&one_line(&card.operation.expression)),
                md_cell(&missing_summary(card)),
                route,
                md_cell(&card.next_action.summary)
            ));
        }
        out.push('\n');
    }

    out.push_str("## Trust boundary\n\n");
    out.push_str("This is static repo posture evidence from unsafe-review cards. It counts open review gaps, not raw unsafe usage. It is ");
    out.push_str(REVIEWCARD_TRUST_BOUNDARY);
    out.push('\n');
    out
}

fn card_location(card: &ReviewCard) -> String {
    format!(
        "{}:{}",
        path_display(&card.site.location.file),
        card.site.location.line
    )
}

fn count_by<F>(output: &AnalyzeOutput, mut value_for_card: F) -> BTreeMap<String, usize>
where
    F: FnMut(&ReviewCard) -> String,
{
    let mut counts = BTreeMap::new();
    for card in &output.cards {
        *counts.entry(value_for_card(card)).or_default() += 1;
    }
    counts
}

fn class_counts(output: &AnalyzeOutput) -> BTreeMap<String, usize> {
    count_by(output, |card| card.class.as_str().to_string())
}

fn operation_counts(output: &AnalyzeOutput) -> BTreeMap<String, usize> {
    count_by(output, |card| card.operation.family.as_str().to_string())
}

fn route_counts(output: &AnalyzeOutput) -> BTreeMap<String, usize> {
    count_by(output, |card| repo_primary_route(card).to_string())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RelatedSinkCluster {
    file: String,
    owner: String,
    line_span: String,
    card_ids: Vec<String>,
    families: Vec<String>,
    classes: Vec<String>,
    routes: Vec<String>,
}

fn render_related_sink_clusters(out: &mut String, output: &AnalyzeOutput) {
    out.push_str("## Related sink clusters\n\n");
    out.push_str("Grouped from existing ReviewCards by source file and inferred owner/helper. This is a report-only triage view, not a call graph and not proof of a shared root cause.\n\n");

    let clusters = related_sink_clusters(output);
    if clusters.is_empty() {
        out.push_str("No multi-card file/owner clusters found.\n\n");
        return;
    }

    out.push_str(
        "| File | Owner/helper | Lines | Cards | Operation families | Classes | Routes |\n",
    );
    out.push_str("|---|---|---:|---|---|---|---|\n");
    for cluster in clusters {
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} | {} | {} | {} |\n",
            md_cell(&cluster.file),
            md_cell(&cluster.owner),
            md_cell(&cluster.line_span),
            render_backtick_string_list(&cluster.card_ids, 4),
            render_backtick_string_list(&cluster.families, 4),
            render_backtick_string_list(&cluster.classes, 4),
            render_backtick_string_list(&cluster.routes, 4)
        ));
    }
    out.push('\n');
}

fn related_sink_clusters(output: &AnalyzeOutput) -> Vec<RelatedSinkCluster> {
    let mut grouped: BTreeMap<(String, String), Vec<&ReviewCard>> = BTreeMap::new();
    for card in &output.cards {
        let file = path_display(&card.site.location.file);
        let owner = card
            .site
            .owner
            .as_deref()
            .unwrap_or(UNKNOWN_OWNER)
            .to_string();
        grouped.entry((file, owner)).or_default().push(card);
    }

    let mut clusters = Vec::new();
    for ((file, owner), mut cards) in grouped {
        if cards.len() < 2 {
            continue;
        }
        cards.sort_by(|left, right| {
            left.site
                .location
                .line
                .cmp(&right.site.location.line)
                .then_with(|| left.id.0.cmp(&right.id.0))
        });
        clusters.push(RelatedSinkCluster {
            file,
            owner,
            line_span: related_line_span(&cards),
            card_ids: cards.iter().map(|card| card.id.to_string()).collect(),
            families: unique_cluster_values(&cards, |card| card.operation.family.as_str()),
            classes: unique_cluster_values(&cards, |card| card.class.as_str()),
            routes: unique_cluster_values(&cards, repo_primary_route),
        });
    }

    clusters.sort_by(|left, right| {
        right
            .card_ids
            .len()
            .cmp(&left.card_ids.len())
            .then_with(|| left.file.cmp(&right.file))
            .then_with(|| left.owner.cmp(&right.owner))
    });
    clusters
}

fn related_line_span(cards: &[&ReviewCard]) -> String {
    let start = cards
        .iter()
        .map(|card| card.site.location.line)
        .min()
        .unwrap_or(0);
    let end = cards
        .iter()
        .map(|card| card.site.location.line)
        .max()
        .unwrap_or(0);
    if start == end {
        start.to_string()
    } else {
        format!("{start}-{end}")
    }
}

fn unique_cluster_values<F>(cards: &[&ReviewCard], value_for_card: F) -> Vec<String>
where
    F: Fn(&ReviewCard) -> &str,
{
    let values = cards
        .iter()
        .map(|card| value_for_card(card).to_string())
        .collect::<BTreeSet<_>>();
    values.into_iter().collect()
}

fn render_backtick_string_list(values: &[String], limit: usize) -> String {
    if values.is_empty() {
        return "`none`".to_string();
    }
    let mut rendered = values
        .iter()
        .take(limit)
        .map(|value| format!("`{}`", md_cell(value)))
        .collect::<Vec<_>>();
    if values.len() > limit {
        rendered.push(format!("+{} more", values.len() - limit));
    }
    rendered.join(", ")
}

fn diff_primary_route(card: &ReviewCard) -> &str {
    card.routes
        .first()
        .map_or(DEFAULT_REVIEW_ROUTE, |route| route.kind.as_str())
}

fn repo_primary_route(card: &ReviewCard) -> &str {
    card.routes
        .first()
        .map_or(DEFAULT_REVIEW_ROUTE, |route| route.kind.as_str())
}

fn agent_handoff_summary(card: &ReviewCard) -> String {
    let projection = agent::repair_queue_projection(card);
    let buckets = repair_queue_buckets(&projection.repair_queue.buckets);
    let bucket_reasons = buckets
        .iter()
        .map(|bucket| repair_queue::bucket_reason(bucket))
        .collect::<Vec<_>>();
    format!(
        "Agent handoff: `{}`; buckets: {}; bucket reasons: {}; readiness reasons: {}",
        projection.agent_readiness.state,
        render_backtick_list(&buckets),
        render_backtick_list(&bucket_reasons),
        projection.agent_readiness.reasons.join("; ")
    )
}

fn repair_queue_buckets(buckets: &[&'static str]) -> Vec<&'static str> {
    REPAIR_QUEUE_BUCKET_ORDER
        .into_iter()
        .filter(|bucket| {
            buckets.iter().any(|candidate| {
                if *candidate == "review_only" {
                    *bucket == "do_not_auto_repair"
                } else {
                    candidate == bucket
                }
            })
        })
        .collect()
}

fn render_backtick_list(values: &[&str]) -> String {
    if values.is_empty() {
        return "`none`".to_string();
    }
    values
        .iter()
        .map(|value| format!("`{value}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn render_counts_table(out: &mut String, label: &str, counts: BTreeMap<String, usize>) {
    if counts.is_empty() {
        out.push_str("No cards to summarize.\n\n");
        return;
    }
    out.push_str(&format!("| {label} | Count |\n"));
    out.push_str("|---|---:|\n");
    for (value, count) in counts {
        out.push_str(&format!("| `{}` | {} |\n", md_cell(&value), count));
    }
    out.push('\n');
}

pub(crate) fn render_pr_summary(output: &AnalyzeOutput) -> String {
    let ranked = confirmation_ranked_cards(output);
    let mut out = String::new();
    render_pr_summary_header(&mut out, output);
    render_pr_summary_build_this_first_lead(&mut out, ranked.first().copied());
    render_pr_summary_reviewer_cockpit(&mut out, ranked.first().copied());
    render_pr_summary_card_table(&mut out, &ranked);
    render_pr_summary_witness_plan(&mut out, &ranked);
    render_pr_summary_trust_boundary(&mut out);
    out
}

/// Presentation-only ranking for pr-summary: cards that are one executable
/// command away from credible confirmation evidence (an available command and
/// a `pending`/`executed` confirmation state) sort first; everything else,
/// including cards that already carry a `confirmed`/`not_reproduced` verdict,
/// keeps the existing priority order behind them. Card identity, cards.json
/// ordering, and comment-plan selection are unchanged.
fn confirmation_ranked_cards(output: &AnalyzeOutput) -> Vec<&ReviewCard> {
    let mut cards = output.cards.iter().collect::<Vec<_>>();
    cards.sort_by_key(|card| confirmation_rank(card));
    cards
}

fn confirmation_rank(card: &ReviewCard) -> u8 {
    let has_command = !card.next_action.verify_commands.is_empty()
        || card
            .routes
            .first()
            .is_some_and(|route| route.command.is_some());
    if has_command && matches!(card.witness.confirmation_state(), "pending" | "executed") {
        0
    } else {
        1
    }
}

fn confirmation_state_label(card: &ReviewCard) -> String {
    let state = card.witness.confirmation_state();
    if state == "not_reproduced" {
        return format!("`{state}` (single run; not a safety claim)");
    }
    format!("`{state}`")
}

fn render_pr_summary_build_this_first_lead(out: &mut String, card: Option<&ReviewCard>) {
    let Some(card) = card else {
        return;
    };
    out.push_str(&format!(
        "BUILD THIS FIRST: {} (cards are ranked by cheapest credible confirmation; this is a confirmation cue, not a verdict)\n\n",
        top_card_build_this_first(card)
    ));
}

/// Bounded summary fragment suitable for `GITHUB_STEP_SUMMARY`.
///
/// Renders the same scope/cards/policy bullets and top-card section as
/// `render_pr_summary` but omits the (potentially large) card table and
/// witness plan, drops the inner H1 since the fragment is embedded under
/// another heading, and points reviewers at the full advisory bundle
/// uploaded as a workflow artifact.
///
/// The top-card selection uses the same `confirmation_ranked_cards` ordering as
/// `render_pr_summary` so both surfaces always name the same headline card.
pub(crate) fn render_github_summary(output: &AnalyzeOutput) -> String {
    let ranked = confirmation_ranked_cards(output);
    let mut out = String::new();
    out.push_str("## unsafe-review advisory summary\n\n");
    render_pr_summary_header_bullets(&mut out, output);
    render_pr_summary_top_card(&mut out, ranked.first().copied());
    render_github_summary_open_next(&mut out);
    out.push_str("---\n\n");
    out.push_str(
        "Full advisory bundle (review-kit.json, cards.json, pr-summary.md, github-summary.md, cards.sarif, comment-plan.json, witness-plan.md, receipt-audit.md, policy-report.json, policy-report.md, manual-candidates.json, manual-repair-queue.json, tokmd-packets.json, lsp.json, repair-queue.json) is attached as the workflow artifact.\n\n",
    );
    out.push_str("> Trust boundary: ");
    out.push_str(REVIEWCARD_TRUST_BOUNDARY);
    out.push('\n');
    out.push_str(
        "> Execution boundary: unsafe-review did not run witnesses, post comments, edit source, run an agent, or enforce blocking policy.\n",
    );
    out
}

fn render_github_summary_open_next(out: &mut String) {
    out.push_str("## Open next\n\n");
    out.push_str("- Review kit manifest: `review-kit.json`\n");
    out.push_str("- Full reviewer cockpit: `pr-summary.md`\n");
    out.push_str("- Machine-readable ReviewCards: `cards.json`\n");
    out.push_str("- Witness routes: `witness-plan.md`\n");
    out.push_str("- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.\n");
    out.push_str(
        "- Policy report: `policy-report.md`; ReviewCard-only; manual candidates are not policy inputs.\n",
    );
    out.push_str("- Manual candidate index: `manual-candidates.json` lists imported advisory candidates separately from ReviewCards.\n");
    out.push_str("- Tokmd packets: `tokmd-packets.json`; tokmd not run.\n");
    out.push_str("- Agent repair queue: `repair-queue.json` is copy-only; no agent was run.\n");
    out.push_str(
        "- Comment budget: `comment-plan.json` is plan-only; no comments were posted.\n\n",
    );
}

fn render_pr_summary_header_bullets(out: &mut String, output: &AnalyzeOutput) {
    out.push_str(&format!(
        "- Scope: `{}`\n",
        match output.scope {
            crate::api::Scope::Diff => "diff",
            crate::api::Scope::Repo => "repo",
        }
    ));
    render_diff_scope_bullet(out, output);
    out.push_str(&format!("- Review cards: {}\n", output.summary.cards));
    out.push_str(&format!(
        "- Open actionable gaps: {}\n",
        output.summary.open_actionable_gaps
    ));
    // Render coverage movement when any movement signal is present.
    let s = &output.summary;
    let has_movement = s.new_gaps > 0
        || s.worsened_gaps > 0
        || s.improved_gaps > 0
        || s.resolved_gaps > 0
        || s.inherited_gaps > 0;
    if has_movement {
        out.push_str(&format!(
            "- Coverage movement: {} new, {} worsened, {} improved (evidence coverage improved; still advisory), {} resolved, {} inherited\n",
            s.new_gaps, s.worsened_gaps, s.improved_gaps, s.resolved_gaps, s.inherited_gaps
        ));
    }
    out.push_str(&format!("- Policy mode: `{}`\n\n", output.policy.as_str()));
}

fn render_diff_scope_bullet(out: &mut String, output: &AnalyzeOutput) {
    if output.summary.changed_files == 0 {
        return;
    }

    out.push_str(&format!(
        "- Diff scope: {} {} changed ({} Rust, {} non-Rust)\n",
        output.summary.changed_files,
        file_word(output.summary.changed_files),
        output.summary.changed_rust_files,
        output.summary.changed_non_rust_files,
    ));
}

fn file_word(count: usize) -> &'static str {
    if count == 1 { "file" } else { "files" }
}

fn render_pr_summary_header(out: &mut String, output: &AnalyzeOutput) {
    out.push_str("# unsafe-review PR summary\n\n");
    render_pr_summary_header_bullets(out, output);
}

fn render_pr_summary_reviewer_cockpit(out: &mut String, top_card: Option<&ReviewCard>) {
    out.push_str("## Reviewer cockpit\n\n");
    if let Some(card) = top_card {
        out.push_str(&format!("- Top card: `{}`\n", card.id));
        out.push_str(&format!("- Class: `{}`\n", card.class.as_str()));
        out.push_str(&format!(
            "- Location: {}:{}\n",
            path_display(&card.site.location.file),
            card.site.location.line
        ));
        out.push_str(&format!(
            "- Operation: `{}`\n",
            one_line(&card.operation.expression)
        ));
        out.push_str(&format!(
            "- Operation family: `{}`\n",
            card.operation.family.as_str()
        ));
        out.push_str(&format!(
            "- Obligation: {}\n",
            primary_obligation_summary(card)
        ));
        out.push_str(&format!("- Proof path: `{}`\n", card.proof_path.as_str()));
        out.push_str(&format!(
            "- Hypothesis to confirm: {}\n",
            top_card_hypothesis(card)
        ));
        out.push_str(&format!(
            "- Build/run this first: {}\n",
            top_card_build_this_first(card)
        ));
        render_minimal_repro_cue(out, card, "- Minimal repro cue:", "  ");
        out.push_str("- Evidence found:\n");
        out.push_str(&format!("  - Contract: {}\n", card.contract.summary));
        out.push_str(&format!(
            "  - Guard/discharge: {}\n",
            card.discharge.summary
        ));
        out.push_str(&format!("  - Reach: {}\n", card.reach.summary));
        out.push_str(&format!("  - Witness: {}\n", card.witness.summary));
        out.push_str(&format!(
            "- Confirmation state: {}\n",
            confirmation_state_label(card)
        ));
        out.push_str(&format!(
            "- Missing/weak evidence: {}\n",
            missing_summary(card)
        ));
        out.push_str(&format!(
            "- Next reviewer action: {}\n",
            card.next_action.summary
        ));
        out.push_str(&format!(
            "- Confirmation step: {}\n",
            top_card_confirmation_step(card)
        ));
        if let Some(route) = card.routes.first() {
            out.push_str(&format!(
                "- Witness route: `{}` because {}\n",
                route.kind.as_str(),
                route.reason
            ));
            if let Some(command) = &route.command {
                out.push_str("  - Suggested command:\n\n");
                push_bash_block(out, command);
            }
        } else {
            out.push_str("- Witness route: no focused witness route was selected; route this to human review.\n");
        }
        out.push_str(
            "- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.\n",
        );
        out.push_str(&format!("- Explain: `unsafe-review explain {}`\n", card.id));
        out.push_str(&format!(
            "- Agent context: `unsafe-review context {} --json`\n",
            card.id
        ));
        out.push_str(&format!("- {}\n", agent_handoff_summary(card)));
        out.push_str("- Trust boundary: ");
        out.push_str(REVIEWCARD_TRUST_BOUNDARY);
        out.push_str("\n\n");
    } else {
        render_no_changed_gaps(out);
        out.push_str(
            "- Receipt audit: `receipt-audit.md` checks saved receipt metadata only; no witness was run.\n\n",
        );
    }
}

fn render_pr_summary_top_card(out: &mut String, top_card: Option<&ReviewCard>) {
    out.push_str("## Top card\n\n");
    if let Some(card) = top_card {
        out.push_str(&format!("- ID: `{}`\n", card.id));
        out.push_str(&format!("- Class: `{}`\n", card.class.as_str()));
        out.push_str(&format!(
            "- Location: {}:{}\n",
            path_display(&card.site.location.file),
            card.site.location.line
        ));
        out.push_str(&format!(
            "- Operation: `{}`\n",
            one_line(&card.operation.expression)
        ));
        out.push_str(&format!(
            "- Operation family: `{}`\n",
            card.operation.family.as_str()
        ));
        out.push_str(&format!("- Proof path: `{}`\n", card.proof_path.as_str()));
        out.push_str(&format!(
            "- Hypothesis to confirm: {}\n",
            top_card_hypothesis(card)
        ));
        out.push_str(&format!(
            "- Build/run this first: {}\n",
            top_card_build_this_first(card)
        ));
        render_minimal_repro_cue(out, card, "- Minimal repro cue:", "  ");
        out.push_str(&format!("- Missing evidence: {}\n", missing_summary(card)));
        if let Some(route) = card.routes.first() {
            out.push_str(&format!(
                "- Primary route: `{}` because {}\n",
                route.kind.as_str(),
                route.reason
            ));
            if let Some(command) = &route.command {
                out.push('\n');
                push_bash_block(out, command);
            }
        }
        out.push_str(&format!("- Next action: {}\n", card.next_action.summary));
        out.push_str(&format!(
            "- Confirmation step: {}\n",
            top_card_confirmation_step(card)
        ));
        out.push_str(&format!("- Explain: `unsafe-review explain {}`\n", card.id));
        out.push_str(&format!(
            "- Agent context: `unsafe-review context {} --json`\n",
            card.id
        ));
        out.push_str(&format!("- {}\n\n", agent_handoff_summary(card)));
    } else {
        render_no_changed_gaps(out);
    }
}

fn top_card_hypothesis(card: &ReviewCard) -> String {
    hypothesis_to_confirm(card)
}

fn top_card_build_this_first(card: &ReviewCard) -> String {
    build_this_first(card).summary
}

fn top_card_confirmation_step(card: &ReviewCard) -> String {
    if let Some(command) = card.next_action.verify_commands.first() {
        return format!(
            "build/run `{}` first for this card, then attach a matching receipt if it confirms the route",
            command
        );
    }
    confirmation_step(card)
}

fn render_minimal_repro_cue(out: &mut String, card: &ReviewCard, label: &str, indent: &str) {
    let cue = minimal_repro(card);
    out.push_str(label);
    out.push('\n');
    for step in cue.steps() {
        out.push_str(indent);
        out.push_str("- ");
        out.push_str(step);
        out.push('\n');
    }
    out.push_str(indent);
    out.push_str("- Limitation: ");
    out.push_str(cue.limitation());
    out.push('\n');
}

fn render_pr_summary_card_table(out: &mut String, ranked_cards: &[&ReviewCard]) {
    out.push_str("## Card table\n\n");

    // Split into specific-operation cards and owner-contract cards.
    // Owner cards are grouped into a summary line to keep the table focused on
    // actionable operation cards. All cards remain in cards.json and in evidence
    // counts; this is a presentation-only grouping.
    let specific_cards: Vec<&&ReviewCard> = ranked_cards
        .iter()
        .filter(|card| !is_owner_contract_card(card))
        .collect();
    let owner_cards: Vec<&&ReviewCard> = ranked_cards
        .iter()
        .filter(|card| is_owner_contract_card(card))
        .collect();

    if specific_cards.is_empty() && owner_cards.is_empty() {
        return;
    }

    out.push_str(
        "| ID | Class | Proof path | Location | Operation family | Operation | Missing evidence | Route | Next action | Confirmation state |\n",
    );
    out.push_str("|---|---|---|---|---|---|---|---|---|---|\n");
    for card in &specific_cards {
        let route = repo_primary_route(card);
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | {} | `{}` | `{}` | {} | `{}` | {} | {} |\n",
            md_cell(&card.id.to_string()),
            card.class.as_str(),
            card.proof_path.as_str(),
            md_cell(&format!(
                "{}:{}",
                path_display(&card.site.location.file),
                card.site.location.line
            )),
            card.operation.family.as_str(),
            md_cell(&one_line(&card.operation.expression)),
            md_cell(&missing_summary(card)),
            route,
            md_cell(&card.next_action.summary),
            confirmation_state_label(card)
        ));
    }

    // Render owner-contract cards as a grouped summary row so the table stays
    // focused on the actionable operation cards. The full list is in cards.json.
    if !owner_cards.is_empty() {
        let unsafe_fn_count = owner_cards.len();
        let location_list: Vec<String> = owner_cards
            .iter()
            .map(|card| {
                format!(
                    "{}:{}",
                    path_display(&card.site.location.file),
                    card.site.location.line
                )
            })
            .collect();
        let location_summary = if location_list.len() <= 3 {
            location_list.join(", ")
        } else {
            format!(
                "{} and {} more",
                location_list[..3].join(", "),
                location_list.len() - 3
            )
        };
        let family_summary = grouped_family_summary(&owner_cards);
        out.push_str(&format!(
            "| (grouped) | owner_contract | — | {} | `{}` | {} owner-contract obligation{} across {} `unsafe fn` {} — see `cards.json` for full list | — | — | — | — |\n",
            md_cell(&location_summary),
            md_cell(&family_summary),
            unsafe_fn_count,
            if unsafe_fn_count == 1 { "" } else { "s" },
            unsafe_fn_count,
            if unsafe_fn_count == 1 { "site" } else { "sites" },
        ));
    }
}

fn is_owner_contract_card(card: &ReviewCard) -> bool {
    matches!(
        card.operation.family,
        OperationFamily::UnsafeDeclaration | OperationFamily::Unknown
    )
}

fn grouped_family_summary(owner_cards: &[&&ReviewCard]) -> String {
    let families = owner_cards
        .iter()
        .map(|card| card.operation.family.as_str())
        .collect::<BTreeSet<_>>();
    families.into_iter().collect::<Vec<_>>().join(", ")
}

fn render_pr_summary_witness_plan(out: &mut String, ranked_cards: &[&ReviewCard]) {
    out.push_str("\n## Witness plan\n\n");
    if ranked_cards.is_empty() {
        out.push_str("No witness route is recommended because no review cards were emitted.\n\n");
        return;
    }

    // Split into specific-operation cards and owner-contract cards.
    // Owner cards are grouped into a summary entry. Their full details are in cards.json.
    let specific_cards: Vec<&&ReviewCard> = ranked_cards
        .iter()
        .filter(|card| !is_owner_contract_card(card))
        .collect();
    let owner_cards: Vec<&&ReviewCard> = ranked_cards
        .iter()
        .filter(|card| is_owner_contract_card(card))
        .collect();

    for card in &specific_cards {
        out.push_str(&format!(
            "- `{}` hypothesis: {}\n",
            card.id,
            top_card_hypothesis(card)
        ));
        out.push_str(&format!(
            "  - Confirmation state: {}\n",
            confirmation_state_label(card)
        ));
        out.push_str(&format!(
            "  - Confirmation step: {}\n",
            top_card_confirmation_step(card)
        ));
        out.push_str(&format!(
            "  - Build/run this first: {}\n",
            top_card_build_this_first(card)
        ));
        render_minimal_repro_cue(out, card, "  - Minimal repro cue:", "    ");
        if let Some(route) = card.routes.first() {
            out.push_str(&format!(
                "  - Route: `{}` because {}\n",
                route.kind.as_str(),
                route.reason
            ));
            if let Some(command) = &route.command {
                out.push_str("\n```bash\n");
                out.push_str(command);
                out.push_str("\n```\n");
            } else {
                out.push_str(
                    "  - No automatic command is available; route this to human review.\n",
                );
            }
        } else {
            out.push_str("  - Route: no witness route was selected; route this to human review.\n");
        }
    }

    // Group owner-contract cards into a single summary entry so the witness plan
    // stays focused on executable witness routes. Full details are in cards.json.
    if !owner_cards.is_empty() {
        out.push_str(&format!(
            "- (grouped) {} owner-contract obligation{} across {} `unsafe fn` {} — see `cards.json` for full list\n",
            owner_cards.len(),
            if owner_cards.len() == 1 { "" } else { "s" },
            owner_cards.len(),
            if owner_cards.len() == 1 { "site" } else { "sites" },
        ));
        out.push_str(
            "  - Route: `human-deep-review` — add a `# Safety` section naming caller obligations for each site.\n",
        );
    }

    out.push('\n');
}

fn render_pr_summary_trust_boundary(out: &mut String) {
    out.push_str("## Trust boundary\n\n");
    out.push_str("This artifact projects existing unsafe-review cards for PR review. It is ");
    out.push_str(REVIEWCARD_TRUST_BOUNDARY);
    out.push('\n');
}

fn render_no_changed_gaps(out: &mut String) {
    out.push_str(NO_CHANGED_GAPS_MESSAGE);
    out.push('\n');
    out.push_str(NO_CHANGED_GAPS_LIMITATION);
    out.push_str("\n\n");
}

pub(crate) fn render_card_detail(card: &ReviewCard) -> String {
    let mut out = String::new();
    out.push_str(&format!("# unsafe-review card `{}`\n\n", card.id));
    out.push_str(&format!("**Class:** `{}`\n\n", card.class.as_str()));
    out.push_str(&format!(
        "**Location:** {}:{}\n\n",
        path_display(&card.site.location.file),
        card.site.location.line
    ));
    out.push_str(&format!(
        "**Operation:** `{}`\n\n",
        card.operation.expression
    ));
    out.push_str(&format!(
        "**Operation family:** `{}`\n\n",
        card.operation.family.as_str()
    ));
    out.push_str(&format!(
        "**Proof path:** `{}`\n\n",
        card.proof_path.as_str()
    ));

    out.push_str("## Why this card exists\n\n");
    out.push_str(&format!(
        "The changed code contains a `{}` unsafe operation that unsafe-review classifies as `{}`.\n\n",
        card.operation.family.as_str(),
        card.class.as_str()
    ));
    if !card.hazards.is_empty() {
        out.push_str("Relevant hazard families:\n\n");
        for hazard in &card.hazards {
            out.push_str(&format!("- `{}`\n", hazard.as_str()));
        }
        out.push('\n');
    }

    out.push_str("## Required safety conditions\n\n");
    for obligation in &card.obligations {
        out.push_str(&format!("- {}\n", obligation.description));
    }

    out.push_str("\n## Evidence found\n\n");
    out.push_str(&format!("- Contract: {}\n", card.contract.summary));
    out.push_str(&format!("- Guard/discharge: {}\n", card.discharge.summary));
    out.push_str(&format!("- Reach: {}\n", card.reach.summary));
    out.push_str("- Reach note: static reach evidence only; it does not prove site execution.\n");
    out.push_str(&format!("- Witness: {}\n", card.witness.summary));
    if !card.obligation_evidence.is_empty() {
        out.push_str("\nObligation evidence matrix:\n\n");
        for evidence in &card.obligation_evidence {
            out.push_str(&format!(
                "- `{}`: contract `{}`, guard `{}`, reach `{}`, witness `{}`\n",
                evidence.obligation.key,
                evidence.contract.state,
                evidence.discharge.state,
                evidence.reach.state,
                evidence.witness.state
            ));
        }
    }

    out.push_str("\n## Evidence missing\n\n");
    if card.missing.is_empty() {
        out.push_str("- No missing evidence recorded for this card.\n");
    } else {
        for missing in &card.missing {
            out.push_str(&format!("- {}\n", missing.message));
        }
    }

    render_resolution_guidance(&mut out, card);
    render_non_resolution_guidance(&mut out);
    render_witness_routes(&mut out, card);
    out.push_str("\n## Trust boundary\n\n");
    push_reviewcard_trust_boundary(&mut out);
    out
}

fn push_reviewcard_trust_boundary(out: &mut String) {
    out.push_str("This is ");
    out.push_str(REVIEWCARD_TRUST_BOUNDARY);
    out.push('\n');
}

fn render_resolution_guidance(out: &mut String, card: &ReviewCard) {
    out.push_str("\n## What would resolve this\n\n");
    out.push_str(&format!("- {}\n", card.next_action.summary));
    if card.next_action.verify_commands.is_empty() {
        out.push_str(
            "- Keep the static limitation explicit if no focused witness route is available.\n",
        );
    } else {
        out.push_str(
            "- Then attach a matching witness receipt only after running a focused command such as:\n",
        );
        for command in &card.next_action.verify_commands {
            out.push('\n');
            push_bash_block(out, command);
        }
    }
}

fn render_non_resolution_guidance(out: &mut String) {
    out.push_str("\n## What would not resolve this\n\n");
    out.push_str("- A `SAFETY:` comment alone does not discharge missing guard evidence.\n");
    out.push_str("- A related test mention is not proof that this unsafe site executed.\n");
    out.push_str("- Do not claim witness proof unless a matching receipt exists.\n");
    out.push_str("- Do not widen unsafe scope, suppress the card, or change unrelated unsafe code to silence this review item.\n");
}

fn render_witness_routes(out: &mut String, card: &ReviewCard) {
    out.push_str("\n## Witness route\n\n");
    if card.routes.is_empty() {
        out.push_str("- No focused witness route was selected; route this to human review.\n");
        return;
    }
    for route in &card.routes {
        out.push_str(&format!("- `{}`: {}\n", route.kind.as_str(), route.reason));
        if let Some(command) = &route.command {
            out.push('\n');
            push_bash_block(out, command);
            out.push('\n');
        }
    }
}

fn push_bash_block(out: &mut String, command: &str) {
    out.push_str("```bash\n");
    out.push_str(command);
    out.push_str("\n```\n");
}

fn missing_summary(card: &ReviewCard) -> String {
    if card.missing.is_empty() {
        return "No missing evidence recorded".to_string();
    }
    card.missing
        .iter()
        .map(|missing| missing.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn primary_obligation_summary(card: &ReviewCard) -> &str {
    card.obligations
        .first()
        .map_or("No safety obligation recorded", |obligation| {
            obligation.description.as_str()
        })
}

fn md_cell(value: &str) -> String {
    one_line(value).replace('|', "\\|")
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use std::path::PathBuf;

    #[test]
    fn markdown_report_projects_operation_and_next_action() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let rendered = render(&output);

        assert!(rendered.contains("# unsafe-review"));
        assert!(rendered.contains(
            "| ID | Class | Proof path | Operation | Hazard | Missing | Route | Next action |"
        ));
        assert!(rendered.contains("`source_route_only`"));
        assert!(rendered.contains("unsafe { ptr.cast::<Header>().read() }"));
        assert!(rendered.contains("Add or expose local guards"));
        assert!(rendered.contains("not memory-safety proof"));
        Ok(())
    }

    #[test]
    fn count_by_builds_expected_frequency_map() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let counts = count_by(&output, |card| card.class.as_str().to_string());

        assert_eq!(counts.values().sum::<usize>(), output.cards.len());
        assert!(counts.values().all(|count| *count > 0));
        Ok(())
    }

    #[test]
    fn card_detail_explains_conditions_missing_evidence_and_routes() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let card = output
            .cards
            .first()
            .ok_or_else(|| "raw pointer fixture should emit a card".to_string())?;
        let rendered = render_card_detail(card);

        assert!(rendered.contains("## Why this card exists"));
        assert!(rendered.contains("## Required safety conditions"));
        assert!(rendered.contains("**Operation:** `unsafe { ptr.cast::<Header>().read() }`"));
        assert!(rendered.contains("**Proof path:** `source_route_only`"));
        assert!(rendered.contains("pointer is aligned for the accessed type"));
        assert!(rendered.contains("## Evidence found"));
        assert!(rendered.contains("Guard/discharge:"));
        assert!(rendered.contains("Obligation evidence matrix:"));
        assert!(rendered.contains("## Evidence missing"));
        assert!(rendered.contains("Missing visible local guard"));
        assert!(rendered.contains("## Witness route"));
        assert!(rendered.contains("Pure-Rust UB-adjacent hazard"));
        assert!(rendered.contains("**Operation family:** `raw_pointer_read`"));
        assert!(rendered.contains("cargo +nightly miri test read_header"));
        assert!(rendered.contains("## What would resolve this"));
        assert!(rendered.contains(
            "Add or expose local guards for these `raw_pointer_read` safety obligations:"
        ));
        assert!(rendered.contains("## What would not resolve this"));
        assert!(
            rendered
                .contains("A `SAFETY:` comment alone does not discharge missing guard evidence.")
        );
        assert!(
            rendered
                .contains("A related test mention is not proof that this unsafe site executed.")
        );
        assert!(rendered.contains("Do not claim witness proof unless a matching receipt exists."));
        assert!(rendered.contains("does not prove site execution"));
        assert!(rendered.contains("## Trust boundary"));
        assert!(rendered.contains("not UB-free status"));
        Ok(())
    }

    #[test]
    fn pr_summary_projects_cards_with_witness_plan_and_trust_boundary() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let rendered = render_pr_summary(&output);
        let card = output
            .cards
            .first()
            .ok_or_else(|| "raw pointer fixture should emit a card".to_string())?;

        assert!(rendered.contains("# unsafe-review PR summary"));
        assert!(rendered.contains(&format!(
            "BUILD THIS FIRST: {}",
            top_card_build_this_first(card)
        )));
        assert!(rendered.contains("this is a confirmation cue, not a verdict"));
        assert!(rendered.contains("- Confirmation state: `pending`"));
        assert!(rendered.contains("  - Confirmation state: `pending`"));
        assert!(rendered.contains("| Next action | Confirmation state |"));
        assert!(rendered.contains("## Reviewer cockpit"));
        assert!(rendered.contains("- Diff scope: 1 file changed (1 Rust, 0 non-Rust)"));
        assert!(rendered.contains(&format!("- Top card: `{}`", card.id)));
        assert!(rendered.contains("## Card table"));
        assert!(rendered.contains("- Operation: `unsafe { ptr.cast::<Header>().read() }`"));
        assert!(rendered.contains("- Operation family: `raw_pointer_read`"));
        assert!(rendered.contains("- Proof path: `source_route_only`"));
        assert!(rendered.contains("- Hypothesis to confirm: static `guard_missing` ReviewCard"));
        assert!(rendered.contains(
            "confirm with external evidence before treating it as observed runtime behavior"
        ));
        assert!(rendered.contains(
            "- Build/run this first: Build/run `cargo +nightly miri test read_header` first for this card"
        ));
        assert!(rendered.contains("- Minimal repro cue:"));
        assert!(rendered.contains(&format!(
            "Confirm ReviewCard `{}` still maps to `unsafe {{ ptr.cast::<Header>().read() }}` at `src/lib.rs:8:5` before upgrading confidence.",
            card.id
        )));
        assert!(
            rendered.contains("Minimal repro cue only; unsafe-review did not run this command")
        );
        assert!(rendered.contains("- Obligation:"));
        assert!(rendered.contains("- Evidence found:"));
        assert!(rendered.contains("  - Guard/discharge:"));
        assert!(rendered.contains("- Missing/weak evidence: Missing visible local guard"));
        assert!(rendered.contains("- Next reviewer action: Add or expose local guards"));
        assert!(rendered.contains(
            "- Confirmation step: build/run `cargo +nightly miri test read_header` first"
        ));
        assert!(rendered.contains("- Witness route: `miri` because Pure-Rust UB-adjacent hazard"));
        assert!(
            rendered
                .contains("| ID | Class | Proof path | Location | Operation family | Operation |")
        );
        assert!(rendered.contains("| `source_route_only` |"));
        assert!(rendered.contains("unsafe { ptr.cast::<Header>().read() }"));
        assert!(rendered.contains("| `raw_pointer_read` |"));
        assert!(rendered.contains("## Witness plan"));
        assert!(rendered.contains(&format!(
            "- `{}` hypothesis: static `guard_missing` ReviewCard",
            card.id
        )));
        assert!(rendered.contains(
            "  - Confirmation step: build/run `cargo +nightly miri test read_header` first"
        ));
        assert!(rendered.contains(
            "  - Build/run this first: Build/run `cargo +nightly miri test read_header` first for this card"
        ));
        assert!(rendered.contains("Open actionable gaps: 1"));
        assert!(rendered.contains("Missing visible local guard"));
        assert!(rendered.contains("cargo +nightly miri test read_header"));
        assert!(rendered.contains(&format!("- Explain: `unsafe-review explain {}`", card.id)));
        assert!(rendered.contains(&format!(
            "- Agent context: `unsafe-review context {} --json`",
            card.id
        )));
        assert!(rendered.contains(
            "- Agent handoff: `ready_for_agent`; buckets: `repairable_by_guard`, `requires_witness_receipt`; bucket reasons: `guard_evidence_missing`, `witness_receipt_missing`; readiness reasons: specific operation family"
        ));
        assert!(rendered.contains("- Receipt audit: `receipt-audit.md`"));
        assert!(rendered.contains("no witness was run"));
        assert!(rendered.contains("not Miri-clean status"));
        assert!(rendered.contains("not a site-execution claim"));
        assert!(rendered.contains("not memory-safety proof"));
        Ok(())
    }

    #[test]
    fn pr_summary_ranks_cards_by_cheapest_credible_confirmation() -> Result<(), String> {
        use crate::domain::WitnessEvidence;

        let mut output = fixture_output("duplicate_raw_pointer_reads")?;
        if output.cards.len() < 2 {
            return Err("duplicate fixture should emit at least two cards".to_string());
        }
        // Give the first card a runtime receipt verdict; it already has
        // evidence, so the still-pending second card should rank first.
        output.cards[0].witness = WitnessEvidence::present("miri receipt imported")
            .with_runtime_executed(true)
            .with_verdict(Some("not_reproduced".to_string()));
        let pending_id = output.cards[1].id.to_string();
        let evidenced_id = output.cards[0].id.to_string();

        let rendered = render_pr_summary(&output);

        assert!(rendered.contains(&format!("- Top card: `{pending_id}`")));
        assert!(rendered.contains("- Confirmation state: `pending`"));
        assert!(
            rendered.contains("`not_reproduced` (single run; not a safety claim)"),
            "not_reproduced lines must carry the single-run qualifier: {rendered}"
        );
        let pending_row = rendered
            .find(&format!("| `{pending_id}` |"))
            .ok_or("pending row missing")?;
        let evidenced_row = rendered
            .find(&format!("| `{evidenced_id}` |"))
            .ok_or("evidenced row missing")?;
        assert!(
            pending_row < evidenced_row,
            "pending card with an executable command must rank before the card with evidence"
        );
        // Presentation-only: cards.json ordering is untouched.
        assert_eq!(output.cards[0].id.to_string(), evidenced_id);
        Ok(())
    }

    #[test]
    fn pr_summary_empty_state_is_sparse_and_nonblocking() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let rendered = render_pr_summary(&output);

        assert!(rendered.contains("Review cards: 0"));
        assert!(rendered.contains("Open actionable gaps: 0"));
        assert!(rendered.contains(NO_CHANGED_GAPS_MESSAGE));
        assert!(rendered.contains(NO_CHANGED_GAPS_LIMITATION));
        assert!(rendered.contains("No witness route is recommended"));
        assert!(rendered.contains("- Receipt audit: `receipt-audit.md`"));
        assert!(rendered.contains("no witness was run"));
        assert!(rendered.contains("not UB-free status"));
        assert!(!rendered.contains("All clear"));
        Ok(())
    }

    #[test]
    fn github_summary_points_to_artifacts_without_full_dump() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let rendered = render_github_summary(&output);
        let card = output
            .cards
            .first()
            .ok_or_else(|| "raw pointer fixture should emit a card".to_string())?;

        assert!(rendered.contains("## unsafe-review advisory summary"));
        assert!(rendered.contains("- Diff scope: 1 file changed (1 Rust, 0 non-Rust)"));
        assert!(rendered.contains("## Top card"));
        assert!(rendered.contains(&format!("- ID: `{}`", card.id)));
        assert!(rendered.contains("- Proof path: `source_route_only`"));
        assert!(rendered.contains("- Hypothesis to confirm: static `guard_missing` ReviewCard"));
        assert!(rendered.contains(
            "- Build/run this first: Build/run `cargo +nightly miri test read_header` first for this card"
        ));
        assert!(rendered.contains("- Minimal repro cue:"));
        assert!(rendered.contains(
            "- Confirmation step: build/run `cargo +nightly miri test read_header` first"
        ));
        assert!(rendered.contains(&format!("- Explain: `unsafe-review explain {}`", card.id)));
        assert!(rendered.contains(&format!(
            "- Agent context: `unsafe-review context {} --json`",
            card.id
        )));
        assert!(rendered.contains("## Open next"));
        assert!(rendered.contains("- Review kit manifest: `review-kit.json`"));
        assert!(rendered.contains("- Full reviewer cockpit: `pr-summary.md`"));
        assert!(rendered.contains("- Machine-readable ReviewCards: `cards.json`"));
        assert!(rendered.contains("- Witness routes: `witness-plan.md`"));
        assert!(rendered.contains("- Receipt audit: `receipt-audit.md`"));
        assert!(rendered.contains("no witness was run"));
        assert!(rendered.contains("`comment-plan.json` is plan-only"));
        assert!(rendered.contains("Full advisory bundle"));
        assert!(rendered.contains("receipt-audit.md"));
        assert!(rendered.contains("unsafe-review did not run witnesses"));
        assert!(rendered.contains("post comments"));
        assert!(rendered.contains("edit source"));
        assert!(rendered.contains("enforce blocking policy"));
        assert!(!rendered.contains("# unsafe-review PR summary"));
        assert!(!rendered.contains("## Card table"));
        assert!(!rendered.contains("## Witness plan"));
        Ok(())
    }

    #[test]
    fn github_summary_uses_ranked_top_card_matching_pr_summary() -> Result<(), String> {
        use crate::domain::WitnessEvidence;

        let mut output = fixture_output("duplicate_raw_pointer_reads")?;
        if output.cards.len() < 2 {
            return Err("duplicate fixture should emit at least two cards".to_string());
        }
        // Give cards[0] a runtime receipt verdict so the still-pending cards[1]
        // ranks first under confirmation_ranked_cards.  Both render_pr_summary
        // and render_github_summary must name the same ranked headline card.
        output.cards[0].witness = WitnessEvidence::present("miri receipt imported")
            .with_runtime_executed(true)
            .with_verdict(Some("not_reproduced".to_string()));
        let ranked_id = output.cards[1].id.to_string();
        let unranked_id = output.cards[0].id.to_string();

        let github = render_github_summary(&output);
        let pr = render_pr_summary(&output);

        // github-summary must name the ranked card in its Top card section.
        assert!(
            github.contains(&format!("- ID: `{ranked_id}`")),
            "github-summary Top card must name ranked card `{ranked_id}`, got:\n{github}"
        );
        assert!(
            !github.contains(&format!("- ID: `{unranked_id}`")),
            "github-summary must not name unranked card `{unranked_id}` as top card"
        );
        // pr-summary Reviewer cockpit names the same ranked card.
        assert!(
            pr.contains(&format!("- Top card: `{ranked_id}`")),
            "pr-summary cockpit must also name ranked card `{ranked_id}`"
        );
        Ok(())
    }

    #[test]
    fn repo_posture_markdown_counts_open_gaps_without_safety_claim() -> Result<(), String> {
        let output = repo_fixture_output("raw_pointer_alignment")?;
        let rendered = render(&output);

        assert!(rendered.contains("# unsafe-review repo posture"));
        assert!(rendered.contains("## Summary"));
        assert!(rendered.contains("| 1 | 1 | 0 | 1 | 0 | 0 | 0 | 0 |"));
        assert!(rendered.contains("## Top classes"));
        assert!(rendered.contains("| `guard_missing` | 1 |"));
        assert!(rendered.contains("## Top operation families"));
        assert!(rendered.contains("| `raw_pointer_read` | 1 |"));
        assert!(rendered.contains("## Witness routes"));
        assert!(rendered.contains("## Related sink clusters"));
        assert!(rendered.contains("No multi-card file/owner clusters found."));
        assert!(rendered.contains(
            "| ID | Class | Proof path | Location | Operation family | Operation | Missing evidence | Route | Next action |"
        ));
        assert!(rendered.contains("| `source_route_only` |"));
        assert!(rendered.contains("src/lib.rs:8"));
        assert!(rendered.contains("unsafe { ptr.cast::<Header>().read() }"));
        assert!(rendered.contains("Add or expose local guards"));
        assert!(rendered.contains("## Trust boundary"));
        assert!(rendered.contains("not raw unsafe usage"));
        assert!(rendered.contains("not UB-free status"));
        Ok(())
    }

    #[test]
    fn repo_posture_markdown_groups_related_sink_clusters() -> Result<(), String> {
        let output = repo_fixture_output("duplicate_raw_pointer_reads")?;
        let rendered = render(&output);

        assert!(rendered.contains("## Related sink clusters"));
        assert!(rendered.contains("report-only triage view"));
        assert!(rendered.contains("not a call graph"));
        assert!(rendered.contains("| File | Owner/helper | Lines | Cards |"));
        assert!(rendered.contains("| `src/lib.rs` | `read_two_headers` | `7-9` |"));
        assert!(rendered.contains("`raw_pointer_read`"));
        assert!(rendered.contains("`guard_missing`"));
        assert!(rendered.contains("`miri`"));
        assert!(rendered.contains("-c1`"));
        assert!(rendered.contains("-c2`"));
        Ok(())
    }

    #[test]
    fn repo_posture_markdown_avoids_score_or_all_clear_wording() -> Result<(), String> {
        for fixture in ["raw_pointer_alignment", "safe_code_no_cards"] {
            let output = repo_fixture_output(fixture)?;
            let rendered = render(&output);
            let lower = rendered.to_ascii_lowercase();

            assert!(rendered.contains("# unsafe-review repo posture"));
            assert!(rendered.contains("open review gaps"));
            assert!(rendered.contains("not raw unsafe usage"));
            assert!(
                !lower.contains("score"),
                "repo posture must report review gaps, not scores: {rendered}"
            );
            for forbidden in [
                "all clear",
                "verified",
                "policy-ready",
                "blocking-ready",
                "safe to use",
                "repo is safe",
                "repository is safe",
            ] {
                assert!(
                    !lower.contains(forbidden),
                    "repo posture must not contain overclaim wording {forbidden:?}: {rendered}"
                );
            }
        }

        Ok(())
    }

    /// Drift-lock: a routeless card renders the same fallback route label on both the
    /// diff markdown table (`diff_primary_route`) and the repo-posture table
    /// (`repo_primary_route`).  Previously diff fell back to `"human"` while repo used
    /// the shared `DEFAULT_REVIEW_ROUTE` const (`"human-deep-review"`).
    #[test]
    fn diff_and_repo_route_fallback_are_identical_for_routeless_card() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let card = output
            .cards
            .first_mut()
            .ok_or_else(|| "fixture should emit one card".to_string())?;
        // Strip all routes so both helpers reach their fallback branch.
        card.routes.clear();

        // Diff table — look for the Route column entry in the card row.
        let diff_rendered = render(&output);
        // Repo table — same card rendered in repo scope.
        let mut repo_output = output.clone();
        repo_output.scope = Scope::Repo;
        let repo_rendered = render(&repo_output);

        // Both should contain DEFAULT_REVIEW_ROUTE ("human-deep-review") as the route.
        assert!(
            diff_rendered.contains(DEFAULT_REVIEW_ROUTE),
            "diff markdown route column must fall back to DEFAULT_REVIEW_ROUTE ({DEFAULT_REVIEW_ROUTE:?}) for a routeless card; rendered:\n{diff_rendered}"
        );
        assert!(
            repo_rendered.contains(DEFAULT_REVIEW_ROUTE),
            "repo-posture markdown route column must fall back to DEFAULT_REVIEW_ROUTE ({DEFAULT_REVIEW_ROUTE:?}) for a routeless card"
        );
        // Explicitly verify neither surface falls back to the old "human" string
        // (which would be a regression to the pre-fix behavior).
        assert!(
            !diff_rendered.contains("| `human` |"),
            "diff markdown must not use bare 'human' fallback for routeless card"
        );
        Ok(())
    }

    /// PR summary groups owner cards into a summary line in the
    /// card table and witness plan, rather than listing each one individually.
    /// The operation cards are still listed individually. cards.json is untouched.
    ///
    /// The `attributed_unsafe_fn_no_duplicate` fixture produces both an owner card
    /// (unsafe_declaration-family) and a specific operation card (RawPointerWrite) so both
    /// grouping and individual listing can be validated in the same render.
    #[test]
    fn pr_summary_groups_owner_cards_in_card_table_and_witness_plan() -> Result<(), String> {
        let output = fixture_output("attributed_unsafe_fn_no_duplicate")?;
        let rendered = render_pr_summary(&output);

        // Operation card is listed individually in the card table.
        assert!(
            rendered.contains("| `raw_pointer_write` |"),
            "operation card must be listed individually in the card table; rendered:\n{rendered}"
        );

        // Owner card is NOT listed individually — it is grouped. The owner card's
        // card ID must not appear as a standalone table row (it appears in the
        // grouped summary row instead). We check the owner card ID is absent as
        // an individual row entry rather than checking for the family name, which
        // legitimately appears in the grouped row.
        let owner_card_id = output
            .cards
            .iter()
            .find(|c| is_owner_contract_card(c))
            .map(|c| c.id.to_string())
            .ok_or_else(|| "fixture must have an owner card".to_string())?;
        assert!(
            !rendered.contains(&format!("| `{owner_card_id}` |")),
            "owner card must not appear as an individual table row; rendered:\n{rendered}"
        );

        // The grouped summary line must be present in the card table.
        assert!(
            rendered.contains("owner-contract obligation"),
            "card table must contain grouped owner-contract summary line; rendered:\n{rendered}"
        );
        assert!(
            rendered.contains("see `cards.json` for full list"),
            "grouped row must reference cards.json; rendered:\n{rendered}"
        );

        // The witness plan must show the grouped owner-contract entry.
        assert!(
            rendered.contains("(grouped)"),
            "witness plan must contain grouped owner card entry; rendered:\n{rendered}"
        );
        assert!(
            rendered.contains("human-deep-review"),
            "grouped owner entry must reference human-deep-review route; rendered:\n{rendered}"
        );

        // The count of cards in the summary header is unchanged (all cards still counted).
        let owner_count = output
            .cards
            .iter()
            .filter(|c| is_owner_contract_card(c))
            .count();
        assert!(
            owner_count > 0,
            "fixture must produce at least one owner card for this test to be meaningful"
        );

        Ok(())
    }

    /// Guardrail: owner cards are still present in cards.json and in the evidence
    /// count — they are not deleted by the grouping. Only the pr-summary card table
    /// and witness plan presentation changes.
    #[test]
    fn pr_summary_grouping_does_not_delete_owner_cards_from_output() -> Result<(), String> {
        let output = fixture_output("attributed_unsafe_fn_no_duplicate")?;

        let owner_cards: Vec<_> = output
            .cards
            .iter()
            .filter(|c| is_owner_contract_card(c))
            .collect();
        let operation_cards: Vec<_> = output
            .cards
            .iter()
            .filter(|c| !is_owner_contract_card(c))
            .collect();

        assert!(
            !owner_cards.is_empty(),
            "fixture must produce at least one owner card"
        );
        assert!(
            !operation_cards.is_empty(),
            "fixture must produce at least one specific operation card"
        );

        // All cards are still present in output.cards (the structured artifact source).
        assert_eq!(
            output.cards.len(),
            owner_cards.len() + operation_cards.len(),
            "all cards must remain in output.cards; none deleted by grouping"
        );

        // The pr-summary is presentation-only: it groups the human display but
        // does not remove cards from the structured output.
        let rendered = render_pr_summary(&output);
        // The operation card is still individually described in the cockpit (top card).
        assert!(
            rendered.contains("raw_pointer_write"),
            "operation card family must appear in pr-summary cockpit; rendered:\n{rendered}"
        );

        Ok(())
    }

    fn fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name);
        analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Diff,
            diff: DiffSource::File(root.join("change.diff")),
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })
    }

    fn repo_fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name);
        analyze(AnalyzeInput {
            root,
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })
    }
}
