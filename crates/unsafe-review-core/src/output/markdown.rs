use crate::api::AnalyzeOutput;
use crate::api::Scope;
use crate::domain::ReviewCard;
use crate::output::agent;
use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};
use crate::util::path_display;
use std::collections::BTreeMap;

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
    out.push_str("| ID | Class | Operation | Hazard | Missing | Route | Next action |\n");
    out.push_str("|---|---|---|---|---|---|---|\n");
    for card in &output.cards {
        let hazard = card.hazards.first().map_or("unknown", |h| h.as_str());
        let missing = card.missing.first().map_or("", |m| m.kind.as_str());
        let route = diff_primary_route(card);
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` | `{}` | {} |\n",
            md_cell(&card.id.to_string()),
            card.class.as_str(),
            md_cell(&one_line(&card.operation.expression)),
            hazard,
            missing,
            route,
            md_cell(&card.next_action.summary)
        ));
    }
    out.push_str("\n## Trust boundary\n\n");
    out.push_str("This is static unsafe contract review. It is not a proof of memory safety and not a Miri result unless a witness receipt is attached.\n");
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

    out.push_str("## Cards\n\n");
    if output.cards.is_empty() {
        out.push_str("No repo-scope unsafe-review cards found.\n\n");
    } else {
        out.push_str(
            "| ID | Class | Operation family | Operation | Missing evidence | Route | Next action |\n",
        );
        out.push_str("|---|---|---|---|---|---|---|\n");
        for card in &output.cards {
            let route = repo_primary_route(card);
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | `{}` | {} | `{}` | {} |\n",
                md_cell(&card.id.to_string()),
                card.class.as_str(),
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
    out.push_str("This is static repo posture evidence from unsafe-review cards. It counts open review gaps, not raw unsafe usage, not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached.\n");
    out
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

fn diff_primary_route(card: &ReviewCard) -> &str {
    card.routes
        .first()
        .map_or("human", |route| route.kind.as_str())
}

fn repo_primary_route(card: &ReviewCard) -> &str {
    card.routes
        .first()
        .map_or(DEFAULT_REVIEW_ROUTE, |route| route.kind.as_str())
}

fn agent_handoff_summary(card: &ReviewCard) -> String {
    let projection = agent::repair_queue_projection(card);
    let buckets = repair_queue_buckets(&projection.repair_queue.buckets);
    format!(
        "Agent handoff: `{}`; buckets: {}; reasons: {}",
        projection.agent_readiness.state,
        render_backtick_list(&buckets),
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
    let mut out = String::new();
    render_pr_summary_header(&mut out, output);
    render_pr_summary_reviewer_cockpit(&mut out, output);
    render_pr_summary_card_table(&mut out, output);
    render_pr_summary_witness_plan(&mut out, output);
    render_pr_summary_trust_boundary(&mut out);
    out
}

/// Bounded summary fragment suitable for `GITHUB_STEP_SUMMARY`.
///
/// Renders the same scope/cards/policy bullets and top-card section as
/// `render_pr_summary` but omits the (potentially large) card table and
/// witness plan, drops the inner H1 since the fragment is embedded under
/// another heading, and points reviewers at the full advisory bundle
/// uploaded as a workflow artifact.
pub(crate) fn render_github_summary(output: &AnalyzeOutput) -> String {
    let mut out = String::new();
    out.push_str("## unsafe-review advisory summary\n\n");
    render_pr_summary_header_bullets(&mut out, output);
    render_pr_summary_top_card(&mut out, output);
    render_github_summary_open_next(&mut out);
    out.push_str("---\n\n");
    out.push_str(
        "Full advisory bundle (review-kit.json, cards.json, pr-summary.md, github-summary.md, cards.sarif, comment-plan.json, witness-plan.md, receipt-audit.md, lsp.json, repair-queue.json) is attached as the workflow artifact.\n\n",
    );
    out.push_str(
        "> Trust boundary: static unsafe contract review only; not memory-safety proof, not UB-free status, not Miri-clean status, and not site-execution proof.\n",
    );
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
    out.push_str(&format!("- Review cards: {}\n", output.summary.cards));
    out.push_str(&format!(
        "- Open actionable gaps: {}\n",
        output.summary.open_actionable_gaps
    ));
    out.push_str(&format!("- Policy mode: `{}`\n\n", output.policy.as_str()));
}

fn render_pr_summary_header(out: &mut String, output: &AnalyzeOutput) {
    out.push_str("# unsafe-review PR summary\n\n");
    out.push_str(&format!(
        "- Scope: `{}`\n",
        match output.scope {
            crate::api::Scope::Diff => "diff",
            crate::api::Scope::Repo => "repo",
        }
    ));
    out.push_str(&format!("- Review cards: {}\n", output.summary.cards));
    out.push_str(&format!(
        "- Open actionable gaps: {}\n",
        output.summary.open_actionable_gaps
    ));
    out.push_str(&format!("- Policy mode: `{}`\n\n", output.policy.as_str()));
}

fn render_pr_summary_reviewer_cockpit(out: &mut String, output: &AnalyzeOutput) {
    out.push_str("## Reviewer cockpit\n\n");
    if let Some(card) = output.cards.first() {
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
        out.push_str("- Evidence found:\n");
        out.push_str(&format!("  - Contract: {}\n", card.contract.summary));
        out.push_str(&format!(
            "  - Guard/discharge: {}\n",
            card.discharge.summary
        ));
        out.push_str(&format!("  - Reach: {}\n", card.reach.summary));
        out.push_str(&format!("  - Witness: {}\n", card.witness.summary));
        out.push_str(&format!(
            "- Missing/weak evidence: {}\n",
            missing_summary(card)
        ));
        out.push_str(&format!(
            "- Next reviewer action: {}\n",
            card.next_action.summary
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
        out.push_str(&format!("- Explain: `unsafe-review explain {}`\n", card.id));
        out.push_str(&format!(
            "- Agent context: `unsafe-review context {} --json`\n",
            card.id
        ));
        out.push_str(&format!("- {}\n", agent_handoff_summary(card)));
        out.push_str("- Trust boundary: static unsafe contract review only; not proof, not UB-free status, not Miri-clean status, and not site-execution proof.\n\n");
    } else {
        render_no_changed_gaps(out);
    }
}

fn render_pr_summary_top_card(out: &mut String, output: &AnalyzeOutput) {
    out.push_str("## Top card\n\n");
    if let Some(card) = output.cards.first() {
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
        out.push_str(&format!("- Explain: `unsafe-review explain {}`\n", card.id));
        out.push_str(&format!(
            "- Agent context: `unsafe-review context {} --json`\n\n",
            card.id
        ));
    } else {
        render_no_changed_gaps(out);
    }
}

fn render_pr_summary_card_table(out: &mut String, output: &AnalyzeOutput) {
    out.push_str("## Card table\n\n");
    out.push_str(
        "| ID | Class | Location | Operation family | Operation | Missing evidence | Route | Next action |\n",
    );
    out.push_str("|---|---|---|---|---|---|---|---|\n");
    for card in &output.cards {
        let route = repo_primary_route(card);
        out.push_str(&format!(
            "| `{}` | `{}` | {} | `{}` | `{}` | {} | `{}` | {} |\n",
            md_cell(&card.id.to_string()),
            card.class.as_str(),
            md_cell(&format!(
                "{}:{}",
                path_display(&card.site.location.file),
                card.site.location.line
            )),
            card.operation.family.as_str(),
            md_cell(&one_line(&card.operation.expression)),
            md_cell(&missing_summary(card)),
            route,
            md_cell(&card.next_action.summary)
        ));
    }
}

fn render_pr_summary_witness_plan(out: &mut String, output: &AnalyzeOutput) {
    out.push_str("\n## Witness plan\n\n");
    if output.cards.is_empty() {
        out.push_str("No witness route is recommended because no review cards were emitted.\n\n");
        return;
    }
    for card in &output.cards {
        if let Some(route) = card.routes.first() {
            out.push_str(&format!(
                "- `{}`: `{}` because {}\n",
                card.id,
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
            out.push_str(&format!(
                "- `{}`: no witness route was selected; route this to human review.\n",
                card.id
            ));
        }
    }
    out.push('\n');
}

fn render_pr_summary_trust_boundary(out: &mut String) {
    out.push_str("## Trust boundary\n\n");
    out.push_str("This artifact projects existing unsafe-review cards for PR review. It is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n");
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
    out.push_str("This is static unsafe contract review. It is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n");
    out
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
        assert!(
            rendered
                .contains("| ID | Class | Operation | Hazard | Missing | Route | Next action |")
        );
        assert!(rendered.contains("unsafe { ptr.cast::<Header>().read() }"));
        assert!(rendered.contains("Add or expose the local guard"));
        assert!(rendered.contains("not a proof of memory safety"));
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
            "Add or expose the local guard that discharges the `raw_pointer_read` safety obligation."
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
        assert!(rendered.contains("## Reviewer cockpit"));
        assert!(rendered.contains(&format!("- Top card: `{}`", card.id)));
        assert!(rendered.contains("## Card table"));
        assert!(rendered.contains("- Operation: `unsafe { ptr.cast::<Header>().read() }`"));
        assert!(rendered.contains("- Operation family: `raw_pointer_read`"));
        assert!(rendered.contains("- Obligation:"));
        assert!(rendered.contains("- Evidence found:"));
        assert!(rendered.contains("  - Guard/discharge:"));
        assert!(rendered.contains("- Missing/weak evidence: Missing visible local guard"));
        assert!(rendered.contains("- Next reviewer action: Add or expose the local guard"));
        assert!(rendered.contains("- Witness route: `miri` because Pure-Rust UB-adjacent hazard"));
        assert!(rendered.contains("| ID | Class | Location | Operation family | Operation |"));
        assert!(rendered.contains("unsafe { ptr.cast::<Header>().read() }"));
        assert!(rendered.contains("| `raw_pointer_read` |"));
        assert!(rendered.contains("## Witness plan"));
        assert!(rendered.contains("Open actionable gaps: 1"));
        assert!(rendered.contains("Missing visible local guard"));
        assert!(rendered.contains("cargo +nightly miri test read_header"));
        assert!(rendered.contains(&format!("- Explain: `unsafe-review explain {}`", card.id)));
        assert!(rendered.contains(&format!(
            "- Agent context: `unsafe-review context {} --json`",
            card.id
        )));
        assert!(rendered.contains(
            "- Agent handoff: `ready`; buckets: `repairable_by_guard`, `requires_witness_receipt`; reasons: specific operation family"
        ));
        assert!(rendered.contains("not Miri-clean status"));
        assert!(rendered.contains("not site-execution proof"));
        assert!(rendered.contains("not a proof of memory safety"));
        assert!(rendered.contains("not a Miri result unless a witness receipt is attached"));
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
        assert!(rendered.contains("## Top card"));
        assert!(rendered.contains(&format!("- ID: `{}`", card.id)));
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
        assert!(rendered.contains(
            "| ID | Class | Operation family | Operation | Missing evidence | Route | Next action |"
        ));
        assert!(rendered.contains("unsafe { ptr.cast::<Header>().read() }"));
        assert!(rendered.contains("Add or expose the local guard"));
        assert!(rendered.contains("## Trust boundary"));
        assert!(rendered.contains("not raw unsafe usage"));
        assert!(rendered.contains("not UB-free status"));
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
