use std::fmt;
use std::path::Path;

use crate::command::{CheckOptions, DiffInput};
use unsafe_review_core::AnalyzeOutput;

type ArtifactRenderer = fn(&AnalyzeOutput) -> String;
type ArtifactSpec<'a> = (&'a str, ArtifactRenderer);

pub(super) fn print_first_pr_report(
    output: &AnalyzeOutput,
    out_dir: &Path,
    root: &Path,
    check: &CheckOptions,
    no_changed_gaps_message: &str,
    no_changed_gaps_limitation: &str,
    artifacts: &[ArtifactSpec<'_>],
) {
    print_first_pr_overview(output, out_dir);
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
    println!("Inspect top card:");
    println!("  {}", explain_command(root, &card.id));
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
}
