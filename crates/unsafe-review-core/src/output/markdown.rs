use crate::api::AnalyzeOutput;
use crate::api::Scope;
use crate::domain::ReviewCard;
use crate::output::markdown_table;
use crate::util::path_display;
use std::collections::BTreeMap;

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
            out.push_str("```bash\n");
            out.push_str(cmd);
            out.push_str("\n```\n\n");
        }
    } else {
        out.push_str("No actionable unsafe-review cards found.\n\n");
    }
    out.push_str("## Cards\n\n");
    out.push_str("| ID | Class | Hazard | Missing | Route |\n");
    out.push_str("|---|---|---|---|---|\n");
    for card in &output.cards {
        let hazard = card.hazards.first().map_or("unknown", |h| h.as_str());
        let missing = card.missing.first().map_or("", |m| m.kind.as_str());
        let route = card.routes.first().map_or("human", |r| r.kind.as_str());
        out.push_str(&format!(
            "| `{}` | `{}` | `{}` | `{}` | `{}` |\n",
            md_cell(&card.id.to_string()),
            md_cell(card.class.as_str()),
            md_cell(hazard),
            md_cell(missing),
            md_cell(route)
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
        out.push_str("| ID | Class | Operation | Missing evidence | Route |\n");
        out.push_str("|---|---|---|---|---|\n");
        for card in &output.cards {
            let route = card
                .routes
                .first()
                .map_or("human-deep-review", |route| route.kind.as_str());
            out.push_str(&format!(
                "| `{}` | `{}` | `{}` | {} | `{}` |\n",
                md_cell(&card.id.to_string()),
                card.class.as_str(),
                card.operation.family.as_str(),
                md_cell(&missing_summary(card)),
                route
            ));
        }
        out.push('\n');
    }

    out.push_str("## Trust boundary\n\n");
    out.push_str("This is static repo posture evidence from unsafe-review cards. It counts open review gaps, not raw unsafe usage, not memory-safety proof, not UB-free status, and not a Miri result unless a witness receipt is attached.\n");
    out
}

fn class_counts(output: &AnalyzeOutput) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for card in &output.cards {
        *counts.entry(card.class.as_str().to_string()).or_default() += 1;
    }
    counts
}

fn operation_counts(output: &AnalyzeOutput) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for card in &output.cards {
        *counts
            .entry(card.operation.family.as_str().to_string())
            .or_default() += 1;
    }
    counts
}

fn route_counts(output: &AnalyzeOutput) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for card in &output.cards {
        let route = card
            .routes
            .first()
            .map_or("human-deep-review", |route| route.kind.as_str());
        *counts.entry(route.to_string()).or_default() += 1;
    }
    counts
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
        out.push_str(&format!("- Missing evidence: {}\n", missing_summary(card)));
        if let Some(route) = card.routes.first() {
            out.push_str(&format!(
                "- Primary route: `{}` because {}\n",
                route.kind.as_str(),
                route.reason
            ));
            if let Some(command) = &route.command {
                out.push_str("\n```bash\n");
                out.push_str(command);
                out.push_str("\n```\n");
            }
        }
        out.push_str(&format!("- Next action: {}\n\n", card.next_action.summary));
    } else {
        out.push_str("No actionable unsafe-review cards found.\n\n");
    }

    out.push_str("## Card table\n\n");
    out.push_str(
        "| ID | Class | Location | Operation | Missing evidence | Route | Next action |\n",
    );
    out.push_str("|---|---|---|---|---|---|---|\n");
    for card in &output.cards {
        let route = card
            .routes
            .first()
            .map_or("human-deep-review", |route| route.kind.as_str());
        out.push_str(&format!(
            "| `{}` | `{}` | {} | `{}` | {} | `{}` | {} |\n",
            md_cell(&card.id.to_string()),
            card.class.as_str(),
            md_cell(&format!(
                "{}:{}",
                path_display(&card.site.location.file),
                card.site.location.line
            )),
            md_cell(&one_line(&card.operation.expression)),
            md_cell(&missing_summary(card)),
            route,
            md_cell(&card.next_action.summary)
        ));
    }

    out.push_str("\n## Witness plan\n\n");
    if output.cards.is_empty() {
        out.push_str("No witness route is recommended because no review cards were emitted.\n\n");
    } else {
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

    out.push_str("## Trust boundary\n\n");
    out.push_str("This artifact projects existing unsafe-review cards for PR review. It is static unsafe contract review, not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n");
    out
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
    out.push_str("## Required safety conditions\n\n");
    for obligation in &card.obligations {
        out.push_str(&format!("- {}\n", obligation.description));
    }
    out.push_str("\n## Hazards\n\n");
    for hazard in &card.hazards {
        out.push_str(&format!("- `{}`\n", hazard.as_str()));
    }
    out.push_str("\n## Evidence\n\n");
    out.push_str(&format!("- Contract: {}\n", card.contract.summary));
    out.push_str(&format!("- Discharge: {}\n", card.discharge.summary));
    out.push_str(&format!("- Reach: {}\n", card.reach.summary));
    out.push_str("- Reach note: static reach evidence only; it does not prove site execution.\n");
    out.push_str(&format!("- Witness: {}\n", card.witness.summary));
    if !card.obligation_evidence.is_empty() {
        out.push_str("\n## Obligation evidence\n\n");
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
    if !card.missing.is_empty() {
        out.push_str("\n## Missing evidence\n\n");
        for missing in &card.missing {
            out.push_str(&format!("- {}\n", missing.message));
        }
    }
    if !card.routes.is_empty() {
        out.push_str("\n## Recommended witness routes\n\n");
        for route in &card.routes {
            out.push_str(&format!("- `{}`: {}\n", route.kind.as_str(), route.reason));
            if let Some(command) = &route.command {
                out.push_str("\n```bash\n");
                out.push_str(command);
                out.push_str("\n```\n");
            }
        }
    }
    out.push_str("\n## Next action\n\n");
    out.push_str(&card.next_action.summary);
    render_resolution_guidance(&mut out, card);
    render_non_resolution_guidance(&mut out);
    out.push_str("\n## Trust boundary\n\n");
    out.push_str("This is static unsafe contract review. It is not a proof of memory safety and not a Miri result unless a witness receipt is attached.\n");
    out
}

fn render_resolution_guidance(out: &mut String, card: &ReviewCard) {
    out.push_str("\n\n## What would resolve this\n\n");
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
            out.push_str("\n```bash\n");
            out.push_str(command);
            out.push_str("\n```\n");
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

fn md_cell(value: &str) -> String {
    markdown_table::cell(value)
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use crate::domain::CardId;
    use std::path::PathBuf;

    #[test]
    fn card_detail_explains_conditions_missing_evidence_and_routes() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let card = output
            .cards
            .first()
            .ok_or_else(|| "raw pointer fixture should emit a card".to_string())?;
        let rendered = render_card_detail(card);

        assert!(rendered.contains("## Required safety conditions"));
        assert!(rendered.contains("pointer is aligned for the accessed type"));
        assert!(rendered.contains("## Missing evidence"));
        assert!(rendered.contains("Missing visible local guard"));
        assert!(rendered.contains("## Recommended witness routes"));
        assert!(rendered.contains("Pure-Rust UB-adjacent hazard"));
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
        Ok(())
    }

    #[test]
    fn pr_summary_projects_cards_with_witness_plan_and_trust_boundary() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let rendered = render_pr_summary(&output);

        assert!(rendered.contains("# unsafe-review PR summary"));
        assert!(rendered.contains("## Top card"));
        assert!(rendered.contains("## Card table"));
        assert!(rendered.contains("## Witness plan"));
        assert!(rendered.contains("Open actionable gaps: 1"));
        assert!(rendered.contains("Missing visible local guard"));
        assert!(rendered.contains("cargo +nightly miri test read_header"));
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
        assert!(rendered.contains("No actionable unsafe-review cards found."));
        assert!(rendered.contains("No witness route is recommended"));
        assert!(rendered.contains("not UB-free status"));
        Ok(())
    }

    #[test]
    fn generic_markdown_card_table_escapes_table_cells() -> Result<(), String> {
        let mut output = fixture_output("raw_pointer_alignment")?;
        let card = output
            .cards
            .first_mut()
            .ok_or_else(|| "raw pointer fixture should emit a card".to_string())?;
        card.id = CardId("UR-pipe|card-c1".to_string());
        let missing = card
            .missing
            .first_mut()
            .ok_or_else(|| "raw pointer fixture should have missing evidence".to_string())?;
        missing.kind = "guard|alignment".to_string();

        let rendered = render(&output);

        assert!(rendered.contains("`UR-pipe\\|card-c1`"));
        assert!(rendered.contains("`guard\\|alignment`"));
        assert!(!rendered.contains("`UR-pipe|card-c1`"));
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
        assert!(rendered.contains("## Trust boundary"));
        assert!(rendered.contains("not raw unsafe usage"));
        assert!(rendered.contains("not UB-free status"));
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
