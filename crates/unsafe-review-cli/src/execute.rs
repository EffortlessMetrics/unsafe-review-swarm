use crate::command::{
    CheckOptions, Command, DiffInput, FirstPrOptions, Format, OutcomeOptions,
    ReceiptTemplateOptions, SavedOutputReceiptOptions,
};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use unsafe_review_core::{
    AnalysisMode, AnalyzeInput, CardId, CargoCarefulReceiptInput, ConcurrencyReceiptInput,
    DiffSource, MiriReceiptInput, PolicyMode, ProofReceiptInput, SanitizerReceiptInput, Scope,
    WITNESS_RECEIPT_SCHEMA_VERSION, WitnessReceipt, analyze, audit_witness_receipts,
    collect_context, compare_outcome_json, evaluate_policy_report, explain_card,
    render_comment_plan, render_human, render_json, render_lsp, render_markdown,
    render_outcome_json, render_outcome_markdown, render_policy_report_json,
    render_policy_report_markdown, render_pr_summary, render_receipt_audit_json,
    render_receipt_audit_markdown, render_sarif, render_witness_plan, validate_witness_receipts,
};

pub(crate) fn execute(command: Command) -> Result<(), String> {
    match command {
        Command::Help => {
            print_help();
            Ok(())
        }
        Command::Version => {
            println!("unsafe-review {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Command::Doctor { root } => doctor(&root),
        Command::Check(options) => run_check(options, Scope::Diff, AnalysisMode::Draft),
        Command::Repo(options) => run_check(options, Scope::Repo, AnalysisMode::Repo),
        Command::Pilot(options) => run_check(options, Scope::Diff, AnalysisMode::Draft),
        Command::FirstPr(options) => first_pr(options),
        Command::Badges { root, out } => badges(&root, &out),
        Command::Explain { root, id, format } => explain(&root, &id, format),
        Command::Context { root, id } => context(&root, &id),
        Command::ReceiptTemplate(options) => receipt_template(options),
        Command::ReceiptValidate { root } => receipt_validate(&root),
        Command::ReceiptAudit(options) => receipt_audit(options),
        Command::ReceiptImportMiri(options) => receipt_import_miri(options),
        Command::ReceiptImportCareful(options) => receipt_import_careful(options),
        Command::ReceiptImportSanitizer(options) => receipt_import_sanitizer(options),
        Command::ReceiptImportConcurrency(options) => receipt_import_concurrency(options),
        Command::ReceiptImportProof(options) => receipt_import_proof(options),
        Command::Outcome(options) => outcome(options),
        Command::PolicyReport(options) => policy_report(options),
    }
}

fn run_check(options: CheckOptions, scope: Scope, mode: AnalysisMode) -> Result<(), String> {
    let diff = diff_source(&options)?;
    let policy = options.policy.clone();
    let output = analyze(AnalyzeInput {
        root: options.root,
        scope,
        diff,
        mode,
        policy,
        include_unchanged_tests: true,
        max_cards: options.max_cards,
    })?;
    let rendered = render_with_format(&output, &options.format);
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        println!("{rendered}");
    }
    enforce_policy(&output)?;
    Ok(())
}

fn first_pr(options: FirstPrOptions) -> Result<(), String> {
    let mut check = options.check;
    check.policy = PolicyMode::Advisory;
    let diff = diff_source(&check)?;
    let output = analyze(AnalyzeInput {
        root: check.root,
        scope: Scope::Diff,
        diff,
        mode: AnalysisMode::Draft,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: check.max_cards,
    })?;

    fs::create_dir_all(&options.out_dir)
        .map_err(|err| format!("create {} failed: {err}", options.out_dir.display()))?;
    write_artifact(&options.out_dir.join("cards.json"), render_json(&output))?;
    write_artifact(
        &options.out_dir.join("pr-summary.md"),
        render_pr_summary(&output),
    )?;
    write_artifact(&options.out_dir.join("cards.sarif"), render_sarif(&output))?;
    write_artifact(
        &options.out_dir.join("comment-plan.json"),
        render_comment_plan(&output),
    )?;
    write_artifact(
        &options.out_dir.join("witness-plan.md"),
        render_witness_plan(&output),
    )?;

    println!("unsafe-review first-pr");
    println!("- Review cards: {}", output.summary.cards);
    println!(
        "- Open actionable gaps: {}",
        output.summary.open_actionable_gaps
    );
    if output.summary.open_actionable_gaps == 0 {
        println!("No changed unsafe-review gaps were found.");
        println!(
            "This does not prove the repo safe, not UB-free status, not a Miri-clean claim, and not proof that any unsafe site executed."
        );
    } else if let Some(card) = output.cards.first() {
        println!("Top action:");
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
    }
    println!("Artifacts:");
    for name in [
        "cards.json",
        "pr-summary.md",
        "cards.sarif",
        "comment-plan.json",
        "witness-plan.md",
    ] {
        println!("  {}", options.out_dir.join(name).display());
    }
    println!(
        "Trust boundary: advisory static review only; did not run witnesses, post comments, edit source, or enforce blocking policy."
    );

    Ok(())
}

fn enforce_policy(output: &unsafe_review_core::AnalyzeOutput) -> Result<(), String> {
    match output.policy {
        PolicyMode::Advisory => Ok(()),
        PolicyMode::NoNewDebt => {
            if output.summary.open_actionable_gaps == 0 {
                Ok(())
            } else {
                Err(format!(
                    "no-new-debt policy found {} open actionable gap(s)",
                    output.summary.open_actionable_gaps
                ))
            }
        }
        PolicyMode::Blocking => Err("blocking policy is not implemented".to_string()),
    }
}

fn write_artifact(path: &Path, rendered: String) -> Result<(), String> {
    ensure_parent_dir(path)?;
    fs::write(path, rendered).map_err(|err| format!("write {} failed: {err}", path.display()))
}

fn diff_source(options: &CheckOptions) -> Result<DiffSource, String> {
    if let Some(diff) = &options.diff {
        return match diff {
            DiffInput::File(path) => Ok(DiffSource::File(resolve_diff_path(&options.root, path))),
            DiffInput::Stdin => read_stdin_diff(),
        };
    }
    if let Some(base) = &options.base {
        let output = ProcessCommand::new("git")
            .arg("diff")
            .arg(format!("{base}...HEAD"))
            .current_dir(&options.root)
            .output()
            .map_err(|err| format!("failed to run git diff: {err}"))?;
        if !output.status.success() {
            return Err(format!(
                "git diff failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        return Ok(DiffSource::Text(
            String::from_utf8_lossy(&output.stdout).into_owned(),
        ));
    }
    Ok(DiffSource::NoneRepoScan)
}

fn read_stdin_diff() -> Result<DiffSource, String> {
    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .map_err(|err| format!("read diff from stdin failed: {err}"))?;
    Ok(DiffSource::Text(text))
}

fn resolve_diff_path(root: &Path, path: &Path) -> PathBuf {
    if path.exists() || path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}

fn ensure_parent_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("create {} failed: {err}", parent.display()))?;
    }
    Ok(())
}

fn render_with_format(output: &unsafe_review_core::AnalyzeOutput, format: &Format) -> String {
    match format {
        Format::Human => render_human(output),
        Format::Json => render_json(output),
        Format::Markdown => render_markdown(output),
        Format::PrSummary => render_pr_summary(output),
        Format::Sarif => render_sarif(output),
        Format::CommentPlan => render_comment_plan(output),
        Format::Lsp => render_lsp(output),
        Format::WitnessPlan => render_witness_plan(output),
    }
}

fn doctor(root: &Path) -> Result<(), String> {
    if !root.is_dir() {
        return Err(format!("root {} is not a directory", root.display()));
    }
    let git_available = tool_available("git");
    let git_repo = git_available && git_root_status(root).is_some();
    let base_ref_available = git_repo && git_ref_available(root, "origin/main");

    println!("unsafe-review doctor");
    println!("workspace root: {}", root.display());
    println!("git command: {}", yes_no(git_available));
    println!("git repository: {}", yes_no(git_repo));
    println!("base ref origin/main: {}", yes_no(base_ref_available));
    println!();
    println!("Witness tool signals");
    println!("miri: {}", yes_no(cargo_subcommand_available("miri")));
    println!(
        "cargo-careful: {}",
        yes_no(cargo_subcommand_available("careful") || tool_available("cargo-careful"))
    );
    println!("sanitizers: configure externally with the appropriate Rust toolchain and RUSTFLAGS");
    println!(
        "loom: {}",
        cargo_manifest_hint(root, "loom")
            .unwrap_or("no Cargo.toml dependency hint detected".to_string())
    );
    println!(
        "shuttle: {}",
        cargo_manifest_hint(root, "shuttle")
            .unwrap_or("no Cargo.toml dependency hint detected".to_string())
    );
    println!("kani: {}", yes_no(tool_available("kani")));
    println!("crux: {}", yes_no(tool_available("crux")));
    println!();
    println!("policy: advisory by default");
    println!("witness execution: not run by doctor or by default");
    println!("trust boundary: static review evidence, not soundness proof");
    Ok(())
}

fn tool_available(name: &str) -> bool {
    ProcessCommand::new(name).arg("--version").output().is_ok()
}

fn cargo_subcommand_available(subcommand: &str) -> bool {
    ProcessCommand::new("cargo")
        .arg(subcommand)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn git_root_status(root: &Path) -> Option<String> {
    let output = ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--show-toplevel")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

fn git_ref_available(root: &Path, reference: &str) -> bool {
    ProcessCommand::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("--verify")
        .arg(reference)
        .output()
        .is_ok_and(|output| output.status.success())
}

fn cargo_manifest_hint(root: &Path, name: &str) -> Option<String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).ok()?;
    if text.contains(name) {
        Some("Cargo.toml dependency hint detected".to_string())
    } else {
        None
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn badges(root: &Path, out: &Path) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|err| format!("create {} failed: {err}", out.display()))?;
    let output = analyze(AnalyzeInput {
        root: root.to_path_buf(),
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Repo,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: None,
    })?;
    let color = if output.summary.open_actionable_gaps == 0 {
        "green"
    } else if output.summary.open_actionable_gaps < 10 {
        "yellow"
    } else {
        "orange"
    };
    let main = format!(
        "{{\n  \"schemaVersion\": 1,\n  \"label\": \"unsafe-review\",\n  \"message\": \"{} open gaps\",\n  \"color\": \"{}\"\n}}\n",
        output.summary.open_actionable_gaps, color
    );
    let plus = format!(
        "{{\n  \"schemaVersion\": 1,\n  \"label\": \"unsafe-review+\",\n  \"message\": \"{} contract / {} guard / {} witness\",\n  \"color\": \"{}\"\n}}\n",
        output.summary.contract_missing,
        output.summary.guard_missing,
        output.summary.guarded_unwitnessed,
        color
    );
    fs::write(out.join("unsafe-review.json"), main)
        .map_err(|err| format!("write badge failed: {err}"))?;
    fs::write(out.join("unsafe-review-plus.json"), plus)
        .map_err(|err| format!("write badge failed: {err}"))?;
    println!("wrote badges to {}", out.display());
    Ok(())
}

fn explain(root: &Path, id: &str, format: Format) -> Result<(), String> {
    let output = analyze(AnalyzeInput {
        root: root.to_path_buf(),
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Repo,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: None,
    })?;
    let id = CardId(id.to_string());
    let Some(detail) = explain_card(&output, &id) else {
        return Err(format!("card `{id}` not found"));
    };
    match format {
        Format::Json => {
            let Some(packet) = collect_context(&output, &id) else {
                return Err(format!("card `{id}` not found"));
            };
            println!("{packet}");
        }
        _ => println!("{detail}"),
    }
    Ok(())
}

fn context(root: &Path, id: &str) -> Result<(), String> {
    let output = analyze(AnalyzeInput {
        root: root.to_path_buf(),
        scope: Scope::Repo,
        diff: DiffSource::NoneRepoScan,
        mode: AnalysisMode::Repo,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: None,
    })?;
    let id = CardId(id.to_string());
    let Some(packet) = collect_context(&output, &id) else {
        return Err(format!("card `{id}` not found"));
    };
    println!("{packet}");
    Ok(())
}

fn receipt_template(options: ReceiptTemplateOptions) -> Result<(), String> {
    let receipt = WitnessReceipt {
        schema_version: WITNESS_RECEIPT_SCHEMA_VERSION.to_string(),
        card_id: options.card_id,
        tool: options.tool,
        strength: options.strength,
        author: Some(options.author),
        recorded_at: Some(options.recorded_at),
        expires_at: Some(options.expires_at),
        summary: options.summary,
        command: options.command,
        limitations: if options.limitations.is_empty() {
            None
        } else {
            Some(options.limitations)
        },
    };
    receipt.validate()?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn receipt_validate(root: &Path) -> Result<(), String> {
    let count = validate_witness_receipts(root.to_path_buf())?;
    println!("witness receipts: {count} valid");
    Ok(())
}

fn receipt_audit(options: CheckOptions) -> Result<(), String> {
    let scope = if options.base.is_some() || options.diff.is_some() {
        Scope::Diff
    } else {
        Scope::Repo
    };
    let mode = match &scope {
        Scope::Diff => AnalysisMode::Draft,
        Scope::Repo => AnalysisMode::Repo,
    };
    let diff = diff_source(&options)?;
    let report = audit_witness_receipts(AnalyzeInput {
        root: options.root,
        scope,
        diff,
        mode,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: options.max_cards,
    })?;
    let rendered = match options.format {
        Format::Json => render_receipt_audit_json(&report),
        Format::Markdown => render_receipt_audit_markdown(&report),
        _ => return Err("receipt audit only supports json or markdown output".to_string()),
    };
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn outcome(options: OutcomeOptions) -> Result<(), String> {
    let before = fs::read_to_string(&options.before)
        .map_err(|err| format!("read {} failed: {err}", options.before.display()))?;
    let after = fs::read_to_string(&options.after)
        .map_err(|err| format!("read {} failed: {err}", options.after.display()))?;
    let report = compare_outcome_json(&before, &after)?;
    let rendered = match options.format {
        Format::Json => render_outcome_json(&report),
        Format::Markdown => render_outcome_markdown(&report),
        _ => return Err("outcome only supports json or markdown output".to_string()),
    };
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn policy_report(options: CheckOptions) -> Result<(), String> {
    let scope = if options.base.is_some() || options.diff.is_some() {
        Scope::Diff
    } else {
        Scope::Repo
    };
    let mode = match &scope {
        Scope::Diff => AnalysisMode::Draft,
        Scope::Repo => AnalysisMode::Repo,
    };
    let diff = diff_source(&options)?;
    let report = evaluate_policy_report(AnalyzeInput {
        root: options.root,
        scope,
        diff,
        mode,
        policy: PolicyMode::Advisory,
        include_unchanged_tests: true,
        max_cards: options.max_cards,
    })?;
    let rendered = match options.format {
        Format::Json => render_policy_report_json(&report),
        Format::Markdown => render_policy_report_markdown(&report),
        _ => return Err("policy report only supports json or markdown output".to_string()),
    };
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn receipt_import_miri(options: SavedOutputReceiptOptions) -> Result<(), String> {
    let output = fs::read_to_string(&options.log)
        .map_err(|err| format!("read {} failed: {err}", options.log.display()))?;
    let receipt = WitnessReceipt::from_miri_output(MiriReceiptInput {
        card_id: options.card_id,
        output,
        author: options.author,
        recorded_at: options.recorded_at,
        expires_at: options.expires_at,
        command: options.command,
        limitations: options.limitations,
    })?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn receipt_import_careful(options: SavedOutputReceiptOptions) -> Result<(), String> {
    let output = fs::read_to_string(&options.log)
        .map_err(|err| format!("read {} failed: {err}", options.log.display()))?;
    let receipt = WitnessReceipt::from_cargo_careful_output(CargoCarefulReceiptInput {
        card_id: options.card_id,
        output,
        author: options.author,
        recorded_at: options.recorded_at,
        expires_at: options.expires_at,
        command: options.command,
        limitations: options.limitations,
    })?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn receipt_import_sanitizer(options: SavedOutputReceiptOptions) -> Result<(), String> {
    let output = fs::read_to_string(&options.log)
        .map_err(|err| format!("read {} failed: {err}", options.log.display()))?;
    let receipt = WitnessReceipt::from_sanitizer_output(SanitizerReceiptInput {
        card_id: options.card_id,
        tool: options
            .tool
            .ok_or_else(|| "missing value for --tool".to_string())?,
        output,
        author: options.author,
        recorded_at: options.recorded_at,
        expires_at: options.expires_at,
        command: options.command,
        limitations: options.limitations,
    })?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn receipt_import_concurrency(options: SavedOutputReceiptOptions) -> Result<(), String> {
    let output = fs::read_to_string(&options.log)
        .map_err(|err| format!("read {} failed: {err}", options.log.display()))?;
    let receipt = WitnessReceipt::from_concurrency_output(ConcurrencyReceiptInput {
        card_id: options.card_id,
        tool: options
            .tool
            .ok_or_else(|| "missing value for --tool".to_string())?,
        output,
        author: options.author,
        recorded_at: options.recorded_at,
        expires_at: options.expires_at,
        command: options.command,
        limitations: options.limitations,
    })?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn receipt_import_proof(options: SavedOutputReceiptOptions) -> Result<(), String> {
    let output = fs::read_to_string(&options.log)
        .map_err(|err| format!("read {} failed: {err}", options.log.display()))?;
    let receipt = WitnessReceipt::from_proof_output(ProofReceiptInput {
        card_id: options.card_id,
        tool: options
            .tool
            .ok_or_else(|| "missing value for --tool".to_string())?,
        output,
        author: options.author,
        recorded_at: options.recorded_at,
        expires_at: options.expires_at,
        command: options.command,
        limitations: options.limitations,
    })?;
    let rendered = receipt.to_pretty_json()?;
    if let Some(path) = options.out {
        ensure_parent_dir(&path)?;
        fs::write(&path, rendered)
            .map_err(|err| format!("write {} failed: {err}", path.display()))?;
    } else {
        print!("{rendered}");
    }
    Ok(())
}

fn print_help() {
    println!("unsafe-review: cheap unsafe contract review for Rust");
    println!();
    println!("Commands:");
    println!(
        "  check   [--root .] [--base origin/main | --diff file|-] [--format human|json|markdown|pr-summary|sarif|comment-plan|lsp|witness-plan] [--policy advisory|no-new-debt] [--out file]"
    );
    println!(
        "  repo    [--root .] [--format human|json|markdown|pr-summary|sarif|comment-plan|lsp|witness-plan] [--policy advisory|no-new-debt] [--out file]"
    );
    println!(
        "  first-pr [--root .] [--base origin/main|--diff file|-] [--out-dir target/unsafe-review] [--max-cards N]"
    );
    println!("  review  alias for first-pr");
    println!("  pilot   [--root .] [--base origin/main] [--max-cards 5]");
    println!("  badges  [--root .] [--out badges]");
    println!("  explain [--root .] [--json|--format json] <card-id>");
    println!("  context [--root .] [--json|--format json] <card-id>");
    println!(
        "  outcome --before <cards.json> --after <cards.json> [--format json|markdown] [--out file]"
    );
    println!(
        "  policy report [--root .] [--base origin/main|--diff file] [--format json|markdown] [--out file] [--max-cards N]"
    );
    println!(
        "  receipt template <card-id> --tool <lane> --strength <level> --author <owner> --recorded-at <utc> --expires-at <date> [--summary text] [--command text] [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-miri <card-id> --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-careful <card-id> --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-sanitizer <card-id> --tool asan|msan|tsan|lsan --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-concurrency <card-id> --tool loom|shuttle --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
    );
    println!(
        "  receipt import-proof <card-id> --tool kani|crux --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command <cmd> [--limitation text] [--out file]"
    );
    println!("  receipt validate [--root .]");
    println!(
        "  receipt audit [--root .] [--base origin/main|--diff file] [--format json|markdown] [--out file] [--max-cards N]"
    );
    println!("  doctor  [--root .]");
    println!();
    println!("Flags may be passed as `--flag value` or `--flag=value`.");
    println!();
    println!("Trust boundary: static review evidence, not soundness proof.");
}
