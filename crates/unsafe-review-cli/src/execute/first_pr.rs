use std::path::Path;

use unsafe_review_core::AnalyzeOutput;

type ArtifactRenderer = fn(&AnalyzeOutput) -> String;
type ArtifactSpec<'a> = (&'a str, ArtifactRenderer);

pub(super) fn print_first_pr_report(
    output: &AnalyzeOutput,
    out_dir: &Path,
    root: &Path,
    no_changed_gaps_message: &str,
    no_changed_gaps_limitation: &str,
    artifacts: &[ArtifactSpec<'_>],
) {
    print_first_pr_overview(output, out_dir);
    print_top_card_summary(
        output,
        root,
        no_changed_gaps_message,
        no_changed_gaps_limitation,
    );
    print_artifact_paths(out_dir, artifacts);
    print_trust_boundary();
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
    println!("Inspect top card:");
    println!(
        "  unsafe-review explain --root {} {}",
        root.display(),
        card.id
    );
    println!(
        "  unsafe-review context --root {} {} --json",
        root.display(),
        card.id
    );
}

fn print_artifact_paths(out_dir: &Path, artifacts: &[ArtifactSpec<'_>]) {
    println!("Artifacts:");
    for (name, _) in artifacts {
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
