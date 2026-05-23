use crate::command::{CheckOptions, Command, DiffInput, FirstPrOptions, Format, OutcomeOptions};
use std::path::PathBuf;
use unsafe_review_core::PolicyMode;

mod policy;
mod receipt;

pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut rest = args.into_iter();
    let _binary = rest.next();
    let mut rest = rest.collect::<Vec<_>>();
    if rest.is_empty() {
        return Ok(Command::Help);
    }
    if rest
        .iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        return Ok(Command::Help);
    }
    let command = rest.remove(0);
    match command.as_str() {
        "--help" | "-h" | "help" => Ok(Command::Help),
        "--version" | "-V" => Ok(Command::Version),
        "support" => parse_support(rest),
        "doctor" => parse_doctor(rest),
        "check" => parse_check(rest).map(Command::Check),
        "first-pr" | "review" => parse_first_pr(rest).map(Command::FirstPr),
        "repo" => parse_check(rest).map(Command::Repo),
        "pilot" => parse_check(rest).map(|mut options| {
            options.max_cards = Some(options.max_cards.unwrap_or(5));
            Command::Pilot(options)
        }),
        "badges" => parse_badges(rest),
        "explain" => parse_explain(rest),
        "context" => parse_context(rest),
        "outcome" => parse_outcome(rest).map(Command::Outcome),
        "policy" => policy::parse_policy_command(rest),
        "receipt" => receipt::parse_receipt(rest),
        "receipt-template" => receipt::parse_receipt_template(rest).map(Command::ReceiptTemplate),
        "lsp" => Ok(Command::Lsp),
        other => Err(format!(
            "unknown command `{other}`. Run `unsafe-review --help`."
        )),
    }
}

fn parse_support(args: Vec<String>) -> Result<Command, String> {
    if let Some(other) = args.first() {
        return Err(format!("unknown support argument `{other}`"));
    }
    Ok(Command::Support)
}

fn parse_doctor(args: Vec<String>) -> Result<Command, String> {
    let mut root = PathBuf::from(".");
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = parse_path_value(&args, idx, "--root")?;
            }
            arg if arg.starts_with("--root=") => {
                root = parse_inline_path_value(arg, "--root")?;
            }
            other => return Err(format!("unknown doctor argument `{other}`")),
        }
        idx += 1;
    }
    Ok(Command::Doctor { root })
}

fn parse_first_pr(args: Vec<String>) -> Result<FirstPrOptions, String> {
    let mut options = FirstPrOptions::default();
    let mut idx = 0usize;
    while idx < args.len() {
        if let Some(consumed) = check_parse::try_apply_check_arg(&args, idx, &mut options.check)? {
            idx += consumed;
            continue;
        }
        match args[idx].as_str() {
            "--out-dir" => {
                idx += 1;
                options.out_dir = PathBuf::from(value(&args, idx, "--out-dir")?);
            }
            arg if arg.starts_with("--out-dir=") => {
                options.out_dir = PathBuf::from(inline_value(arg, "--out-dir")?);
            }
            other => return Err(format!("unknown first-pr argument `{other}`")),
        }
        idx += 1;
    }
    if options.check.base.is_none() && options.check.diff.is_none() {
        options.check.base = Some("origin/main".to_string());
    }
    validate_check_options(&options.check)?;
    Ok(options)
}

fn parse_check(args: Vec<String>) -> Result<CheckOptions, String> {
    let mut options = CheckOptions::default();
    let mut idx = 0usize;
    while idx < args.len() {
        idx += check_parse::apply_check_arg(&args, idx, &mut options)?;
    }
    validate_check_options(&options)?;
    Ok(options)
}

mod check_parse {
    use super::*;

    pub(super) fn apply_check_arg(
        args: &[String],
        idx: usize,
        options: &mut CheckOptions,
    ) -> Result<usize, String> {
        try_apply_check_arg(args, idx, options)?
            .ok_or_else(|| format!("unknown argument `{}`", args[idx]))
    }

    pub(super) fn try_apply_check_arg(
        args: &[String],
        idx: usize,
        options: &mut CheckOptions,
    ) -> Result<Option<usize>, String> {
        match args[idx].as_str() {
            "--root" => {
                options.root = PathBuf::from(value(args, idx + 1, "--root")?);
                Ok(Some(2))
            }
            arg if arg.starts_with("--root=") => {
                options.root = PathBuf::from(inline_value(arg, "--root")?);
                Ok(Some(1))
            }
            "--base" => {
                options.base = Some(value(args, idx + 1, "--base")?.to_string());
                Ok(Some(2))
            }
            arg if arg.starts_with("--base=") => {
                options.base = Some(inline_value(arg, "--base")?.to_string());
                Ok(Some(1))
            }
            "--diff" => {
                options.diff = Some(parse_diff_input(value(args, idx + 1, "--diff")?));
                Ok(Some(2))
            }
            arg if arg.starts_with("--diff=") => {
                options.diff = Some(parse_diff_input(inline_value(arg, "--diff")?));
                Ok(Some(1))
            }
            "--format" => {
                options.format = parse_format(value(args, idx + 1, "--format")?)?;
                Ok(Some(2))
            }
            arg if arg.starts_with("--format=") => {
                options.format = parse_format(inline_value(arg, "--format")?)?;
                Ok(Some(1))
            }
            "--policy" => {
                options.policy = parse_policy(value(args, idx + 1, "--policy")?)?;
                Ok(Some(2))
            }
            arg if arg.starts_with("--policy=") => {
                options.policy = parse_policy(inline_value(arg, "--policy")?)?;
                Ok(Some(1))
            }
            "--json" => {
                options.format = Format::Json;
                Ok(Some(1))
            }
            "--markdown" => {
                options.format = Format::Markdown;
                Ok(Some(1))
            }
            "--out" => {
                options.out = Some(PathBuf::from(value(args, idx + 1, "--out")?));
                Ok(Some(2))
            }
            arg if arg.starts_with("--out=") => {
                options.out = Some(PathBuf::from(inline_value(arg, "--out")?));
                Ok(Some(1))
            }
            "--max-cards" => {
                options.max_cards = Some(parse_max_cards_arg(args, idx + 1, "--max-cards")?);
                Ok(Some(2))
            }
            arg if arg.starts_with("--max-cards=") => {
                options.max_cards = Some(parse_max_cards_inline(arg, "--max-cards")?);
                Ok(Some(1))
            }
            _ => Ok(None),
        }
    }
}

fn parse_max_cards_arg(args: &[String], idx: usize, flag: &str) -> Result<usize, String> {
    parse_max_cards(value(args, idx, flag)?)
}

fn parse_max_cards_inline(arg: &str, flag: &str) -> Result<usize, String> {
    parse_max_cards(inline_value(arg, flag)?)
}

fn parse_badges(args: Vec<String>) -> Result<Command, String> {
    let mut root = PathBuf::from(".");
    let mut out = PathBuf::from("badges");
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = parse_path_value(&args, idx, "--root")?;
            }
            arg if arg.starts_with("--root=") => {
                root = parse_inline_path_value(arg, "--root")?;
            }
            "--out" => {
                idx += 1;
                out = parse_path_value(&args, idx, "--out")?;
            }
            arg if arg.starts_with("--out=") => {
                out = parse_inline_path_value(arg, "--out")?;
            }
            other => return Err(format!("unknown badges argument `{other}`")),
        }
        idx += 1;
    }
    Ok(Command::Badges { root, out })
}

fn parse_explain(args: Vec<String>) -> Result<Command, String> {
    let mut root = PathBuf::from(".");
    let mut format = Format::Markdown;
    let mut id: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = parse_path_value(&args, idx, "--root")?;
            }
            arg if arg.starts_with("--root=") => {
                root = parse_inline_path_value(arg, "--root")?;
            }
            "--format" => {
                idx += 1;
                format = parse_format(value(&args, idx, "--format")?)?;
            }
            arg if arg.starts_with("--format=") => {
                format = parse_format(inline_value(arg, "--format")?)?;
            }
            "--json" => format = Format::Json,
            "--markdown" => format = Format::Markdown,
            value if value.starts_with('-') => {
                return Err(format!("unknown explain argument `{value}`"));
            }
            value => set_card_id(&mut id, value)?,
        }
        idx += 1;
    }
    Ok(Command::Explain {
        root,
        id: id.ok_or_else(|| "missing card id".to_string())?,
        format,
    })
}

fn parse_context(args: Vec<String>) -> Result<Command, String> {
    let mut root = PathBuf::from(".");
    let mut id: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                root = parse_path_value(&args, idx, "--root")?;
            }
            arg if arg.starts_with("--root=") => {
                root = parse_inline_path_value(arg, "--root")?;
            }
            "--json" => {}
            "--format" => {
                idx += 1;
                let raw = value(&args, idx, "--format")?;
                if parse_format(raw)? != Format::Json {
                    return Err("context only supports json output".to_string());
                }
            }
            arg if arg.starts_with("--format=") => {
                if parse_format(inline_value(arg, "--format")?)? != Format::Json {
                    return Err("context only supports json output".to_string());
                }
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown context argument `{value}`"));
            }
            value => set_card_id(&mut id, value)?,
        }
        idx += 1;
    }
    Ok(Command::Context {
        root,
        id: id.ok_or_else(|| "missing card id".to_string())?,
    })
}

fn parse_outcome(args: Vec<String>) -> Result<OutcomeOptions, String> {
    let mut before: Option<PathBuf> = None;
    let mut after: Option<PathBuf> = None;
    let mut format = Format::Json;
    let mut out: Option<PathBuf> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--before" => {
                idx += 1;
                before = Some(PathBuf::from(value(&args, idx, "--before")?));
            }
            arg if arg.starts_with("--before=") => {
                before = Some(PathBuf::from(inline_value(arg, "--before")?));
            }
            "--after" => {
                idx += 1;
                after = Some(PathBuf::from(value(&args, idx, "--after")?));
            }
            arg if arg.starts_with("--after=") => {
                after = Some(PathBuf::from(inline_value(arg, "--after")?));
            }
            "--format" => {
                idx += 1;
                format = parse_outcome_format(value(&args, idx, "--format")?)?;
            }
            arg if arg.starts_with("--format=") => {
                format = parse_outcome_format(inline_value(arg, "--format")?)?;
            }
            "--json" => format = Format::Json,
            "--markdown" => format = Format::Markdown,
            "--out" => {
                idx += 1;
                out = Some(PathBuf::from(value(&args, idx, "--out")?));
            }
            arg if arg.starts_with("--out=") => {
                out = Some(PathBuf::from(inline_value(arg, "--out")?));
            }
            other => return Err(format!("unknown outcome argument `{other}`")),
        }
        idx += 1;
    }
    Ok(OutcomeOptions {
        before: before.ok_or_else(|| "missing value for --before".to_string())?,
        after: after.ok_or_else(|| "missing value for --after".to_string())?,
        format,
        out,
    })
}

fn normalize_report_format(
    format: Format,
    validate: impl FnOnce(Format) -> Result<Format, String>,
) -> Result<Format, String> {
    if matches!(format, Format::Human) {
        return Ok(Format::Json);
    }
    validate(format)
}

fn require_advisory_policy(options: &CheckOptions, message: &str) -> Result<(), String> {
    if options.policy == PolicyMode::Advisory {
        Ok(())
    } else {
        Err(message.to_string())
    }
}

fn parse_outcome_format(raw: &str) -> Result<Format, String> {
    json_or_markdown_format(parse_format(raw)?, "outcome")
}

fn parse_path_value(args: &[String], idx: usize, flag: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(value(args, idx, flag)?))
}

fn parse_inline_path_value(arg: &str, flag: &str) -> Result<PathBuf, String> {
    Ok(PathBuf::from(inline_value(arg, flag)?))
}

fn json_or_markdown_format(format: Format, command_name: &str) -> Result<Format, String> {
    match format {
        Format::Json => Ok(Format::Json),
        Format::Markdown => Ok(Format::Markdown),
        other => Err(format!(
            "{command_name} only supports json or markdown output, got `{}`",
            format_name(&other)
        )),
    }
}

fn parse_diff_input(raw: &str) -> DiffInput {
    if raw == "-" {
        DiffInput::Stdin
    } else {
        DiffInput::File(PathBuf::from(raw))
    }
}

fn validate_check_options(options: &CheckOptions) -> Result<(), String> {
    if options.base.is_some() && options.diff.is_some() {
        return Err("choose only one of --base or --diff".to_string());
    }
    Ok(())
}

fn validate_required_cli_value(value: &str, flag: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("missing value for {flag}"))
    } else {
        Ok(())
    }
}

pub(super) fn parse_receipt_audit_format(format: Format) -> Result<Format, String> {
    json_or_markdown_format(format, "receipt audit")
}

fn set_card_id(id: &mut Option<String>, value: &str) -> Result<(), String> {
    if id.replace(value.to_string()).is_some() {
        return Err("expected exactly one card id".to_string());
    }
    Ok(())
}

fn parse_max_cards(raw: &str) -> Result<usize, String> {
    raw.parse::<usize>()
        .map_err(|err| format!("invalid --max-cards `{raw}`: {err}"))
}

fn parse_format(raw: &str) -> Result<Format, String> {
    match raw {
        "human" => Ok(Format::Human),
        "json" | "repo-json" => Ok(Format::Json),
        "markdown" | "md" => Ok(Format::Markdown),
        "pr-summary" | "github-summary" | "github-markdown" => Ok(Format::PrSummary),
        "sarif" => Ok(Format::Sarif),
        "comment-plan" | "comments" => Ok(Format::CommentPlan),
        "lsp" | "lsp-json" | "editor-json" => Ok(Format::Lsp),
        "witness-plan" | "witness" | "route-plan" => Ok(Format::WitnessPlan),
        other => Err(format!("unknown format `{other}`")),
    }
}

fn parse_policy(raw: &str) -> Result<PolicyMode, String> {
    match raw {
        "advisory" => Ok(PolicyMode::Advisory),
        "no-new-debt" | "no_new_debt" => Ok(PolicyMode::NoNewDebt),
        "blocking" => Err("blocking policy is not implemented".to_string()),
        other => Err(format!("unknown policy `{other}`")),
    }
}

fn value<'a>(args: &'a [String], idx: usize, flag: &str) -> Result<&'a str, String> {
    let Some(value) = args.get(idx).map(|value| value.as_str()) else {
        return Err(format!("missing value for {flag}"));
    };
    if value != "-" && value.starts_with('-') {
        return Err(format!("missing value for {flag}"));
    }
    Ok(value)
}

fn inline_value<'a>(arg: &'a str, flag: &str) -> Result<&'a str, String> {
    let Some(value) = arg
        .strip_prefix(flag)
        .and_then(|rest| rest.strip_prefix('='))
    else {
        return Err(format!("missing value for {flag}"));
    };
    if value.is_empty() {
        return Err(format!("missing value for {flag}"));
    }
    Ok(value)
}

fn format_name(format: &Format) -> &'static str {
    match format {
        Format::Human => "human",
        Format::Json => "json",
        Format::Markdown => "markdown",
        Format::PrSummary => "pr-summary",
        Format::Sarif => "sarif",
        Format::CommentPlan => "comment-plan",
        Format::Lsp => "lsp",
        Format::WitnessPlan => "witness-plan",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_accepts_non_vec_iterators() {
        let args = ["unsafe-review", "--version"]
            .into_iter()
            .map(str::to_string);
        let command = parse(args);
        assert_eq!(command, Ok(Command::Version));
    }

    #[test]
    fn parses_pr_summary_format_for_check() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "check", "--format", "pr-summary"]))?;
        let Command::Check(options) = command else {
            return Err("expected check command".to_string());
        };
        assert_eq!(options.format, Format::PrSummary);
        Ok(())
    }

    #[test]
    fn parses_help_flag_after_subcommand_as_help() -> Result<(), String> {
        assert_eq!(
            parse(args(["unsafe-review", "check", "--help"]))?,
            Command::Help
        );
        assert_eq!(
            parse(args(["unsafe-review", "receipt", "audit", "-h"]))?,
            Command::Help
        );
        Ok(())
    }

    #[test]
    fn parses_support_command() -> Result<(), String> {
        assert_eq!(parse(args(["unsafe-review", "support"]))?, Command::Support);
        assert_eq!(
            parse(args(["unsafe-review", "support", "--root", "."])),
            Err("unknown support argument `--root`".to_string())
        );
        Ok(())
    }

    #[test]
    fn parses_github_summary_alias_for_check() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "check",
            "--format",
            "github-summary",
        ]))?;
        let Command::Check(options) = command else {
            return Err("expected check command".to_string());
        };
        assert_eq!(options.format, Format::PrSummary);
        Ok(())
    }

    #[test]
    fn parses_sarif_format_for_check() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "check", "--format", "sarif"]))?;
        let Command::Check(options) = command else {
            return Err("expected check command".to_string());
        };
        assert_eq!(options.format, Format::Sarif);
        Ok(())
    }

    #[test]
    fn parses_first_pr_bundle_defaults_to_origin_main() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "first-pr",
            "--out-dir=target/review",
        ]))?;
        let Command::FirstPr(options) = command else {
            return Err("expected first-pr command".to_string());
        };
        assert_eq!(options.check.root, PathBuf::from("."));
        assert_eq!(options.check.base, Some("origin/main".to_string()));
        assert_eq!(options.check.diff, None);
        assert_eq!(options.check.policy, PolicyMode::Advisory);
        assert_eq!(options.out_dir, PathBuf::from("target/review"));
        Ok(())
    }

    #[test]
    fn parses_review_bundle_alias_with_diff() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "review",
            "--root=fixtures/raw_pointer_alignment",
            "--diff=change.diff",
            "--max-cards=3",
        ]))?;
        let Command::FirstPr(options) = command else {
            return Err("expected first-pr command".to_string());
        };
        assert_eq!(
            options.check.root,
            PathBuf::from("fixtures/raw_pointer_alignment")
        );
        assert_eq!(
            options.check.diff,
            Some(DiffInput::File(PathBuf::from("change.diff")))
        );
        assert_eq!(options.check.base, None);
        assert_eq!(options.check.max_cards, Some(3));
        assert_eq!(options.out_dir, PathBuf::from("target/unsafe-review"));
        Ok(())
    }

    #[test]
    fn parses_comment_plan_format_for_check() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "check", "--format", "comment-plan"]))?;
        let Command::Check(options) = command else {
            return Err("expected check command".to_string());
        };
        assert_eq!(options.format, Format::CommentPlan);
        Ok(())
    }

    #[test]
    fn parses_lsp_command() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "lsp"]))?;
        assert_eq!(command, Command::Lsp);
        Ok(())
    }

    #[test]
    fn parses_lsp_format_for_check() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "check", "--format", "lsp"]))?;
        let Command::Check(options) = command else {
            return Err("expected check command".to_string());
        };
        assert_eq!(options.format, Format::Lsp);
        Ok(())
    }

    #[test]
    fn parses_witness_plan_format_for_check() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "check", "--format", "witness-plan"]))?;
        let Command::Check(options) = command else {
            return Err("expected check command".to_string());
        };
        assert_eq!(options.format, Format::WitnessPlan);
        Ok(())
    }

    #[test]
    fn parses_no_new_debt_policy_for_check() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "check", "--policy", "no-new-debt"]))?;
        let Command::Check(options) = command else {
            return Err("expected check command".to_string());
        };
        assert_eq!(options.policy, PolicyMode::NoNewDebt);
        Ok(())
    }

    #[test]
    fn rejects_unimplemented_blocking_policy() {
        let command = parse(args(["unsafe-review", "check", "--policy=blocking"]));

        assert_eq!(
            command,
            Err("blocking policy is not implemented".to_string())
        );
    }

    #[test]
    fn parses_equals_style_artifact_flags_for_check() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "check",
            "--root=fixtures/raw_pointer_deref",
            "--diff=-",
            "--format=sarif",
            "--out=target/unsafe-review/cards.sarif",
            "--max-cards=7",
        ]))?;
        let Command::Check(options) = command else {
            return Err("expected check command".to_string());
        };

        assert_eq!(options.root, PathBuf::from("fixtures/raw_pointer_deref"));
        assert_eq!(options.diff, Some(DiffInput::Stdin));
        assert_eq!(options.format, Format::Sarif);
        assert_eq!(
            options.out,
            Some(PathBuf::from("target/unsafe-review/cards.sarif"))
        );
        assert_eq!(options.max_cards, Some(7));
        Ok(())
    }

    #[test]
    fn parses_stdin_diff_for_check() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "check", "--diff", "-", "--json"]))?;
        let Command::Check(options) = command else {
            return Err("expected check command".to_string());
        };

        assert_eq!(options.diff, Some(DiffInput::Stdin));
        assert_eq!(options.format, Format::Json);
        Ok(())
    }

    #[test]
    fn rejects_conflicting_diff_sources_for_check() {
        let command = parse(args([
            "unsafe-review",
            "check",
            "--base",
            "origin/main",
            "--diff",
            "change.diff",
        ]));

        assert_eq!(
            command,
            Err("choose only one of --base or --diff".to_string())
        );
    }

    #[test]
    fn rejects_missing_values_when_next_argument_is_a_flag() {
        let diff = parse(args(["unsafe-review", "check", "--diff", "--json"]));
        let format = parse(args(["unsafe-review", "check", "--format", "--out"]));

        assert_eq!(diff, Err("missing value for --diff".to_string()));
        assert_eq!(format, Err("missing value for --format".to_string()));
    }

    #[test]
    fn parses_context_json_alias() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "context", "--json", "UR-card"]))?;

        assert_eq!(
            command,
            Command::Context {
                root: PathBuf::from("."),
                id: "UR-card".to_string()
            }
        );
        Ok(())
    }

    #[test]
    fn parses_outcome_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "outcome",
            "--before",
            "target/before.json",
            "--after=target/after.json",
            "--format",
            "markdown",
            "--out",
            "target/outcome.md",
        ]))?;

        let Command::Outcome(options) = command else {
            return Err("expected outcome command".to_string());
        };
        assert_eq!(options.before, PathBuf::from("target/before.json"));
        assert_eq!(options.after, PathBuf::from("target/after.json"));
        assert_eq!(options.format, Format::Markdown);
        assert_eq!(options.out, Some(PathBuf::from("target/outcome.md")));
        Ok(())
    }

    #[test]
    fn outcome_rejects_non_outcome_format() {
        let command = parse(args([
            "unsafe-review",
            "outcome",
            "--before",
            "target/before.json",
            "--after",
            "target/after.json",
            "--format",
            "sarif",
        ]));

        assert_eq!(
            command,
            Err("outcome only supports json or markdown output, got `sarif`".to_string())
        );
    }

    #[test]
    fn outcome_requires_before_and_after() {
        let command = parse(args([
            "unsafe-review",
            "outcome",
            "--before",
            "before.json",
        ]));

        assert_eq!(command, Err("missing value for --after".to_string()));
    }

    #[test]
    fn parses_equals_style_explain_and_context_flags() -> Result<(), String> {
        let explain = parse(args([
            "unsafe-review",
            "explain",
            "--root=fixtures/raw_pointer_deref",
            "--format=json",
            "UR-card",
        ]))?;
        assert_eq!(
            explain,
            Command::Explain {
                root: PathBuf::from("fixtures/raw_pointer_deref"),
                id: "UR-card".to_string(),
                format: Format::Json,
            }
        );

        let context = parse(args([
            "unsafe-review",
            "context",
            "--root=fixtures/raw_pointer_deref",
            "--format=json",
            "UR-card",
        ]))?;
        assert_eq!(
            context,
            Command::Context {
                root: PathBuf::from("fixtures/raw_pointer_deref"),
                id: "UR-card".to_string(),
            }
        );
        Ok(())
    }

    #[test]
    fn parses_equals_style_doctor_and_badges_flags() -> Result<(), String> {
        let doctor = parse(args(["unsafe-review", "doctor", "--root=fixtures"]))?;
        assert_eq!(
            doctor,
            Command::Doctor {
                root: PathBuf::from("fixtures"),
            }
        );

        let badges = parse(args([
            "unsafe-review",
            "badges",
            "--root=fixtures",
            "--out=target/badges",
        ]))?;
        assert_eq!(
            badges,
            Command::Badges {
                root: PathBuf::from("fixtures"),
                out: PathBuf::from("target/badges"),
            }
        );
        Ok(())
    }

    #[test]
    fn rejects_non_json_context_format() {
        let command = parse(args([
            "unsafe-review",
            "context",
            "--format",
            "markdown",
            "UR-card",
        ]));

        assert_eq!(
            command,
            Err("context only supports json output".to_string())
        );
    }

    #[test]
    fn rejects_duplicate_card_ids() {
        let explain = parse(args(["unsafe-review", "explain", "UR-one", "UR-two"]));
        let context = parse(args(["unsafe-review", "context", "UR-one", "UR-two"]));

        assert_eq!(explain, Err("expected exactly one card id".to_string()));
        assert_eq!(context, Err("expected exactly one card id".to_string()));
    }

    #[test]
    fn parses_receipt_template_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "template",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--tool=miri",
            "--strength=ran",
            "--author",
            "core/fixtures",
            "--recorded-at",
            "2026-05-18T00:00:00Z",
            "--expires-at=2026-08-18",
            "--summary",
            "focused witness passed",
            "--command",
            "cargo +nightly miri test read_header",
            "--limitation",
            "fixture only",
            "--out",
            "target/receipt.json",
        ]))?;

        let Command::ReceiptTemplate(options) = command else {
            return Err("expected receipt template command".to_string());
        };
        assert_eq!(options.tool, "miri");
        assert_eq!(options.strength, "ran");
        assert_eq!(options.author, "core/fixtures");
        assert_eq!(options.recorded_at, "2026-05-18T00:00:00Z");
        assert_eq!(options.expires_at, "2026-08-18");
        assert_eq!(options.summary.as_deref(), Some("focused witness passed"));
        assert_eq!(
            options.command.as_deref(),
            Some("cargo +nightly miri test read_header")
        );
        assert_eq!(options.limitations, vec!["fixture only".to_string()]);
        assert_eq!(options.out, Some(PathBuf::from("target/receipt.json")));
        Ok(())
    }

    #[test]
    fn parses_receipt_import_miri_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "import-miri",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--log",
            "fixtures/raw_pointer_alignment_receipted/miri.success.log",
            "--author",
            "core/fixtures",
            "--recorded-at",
            "2026-05-18T00:00:00Z",
            "--expires-at",
            "2026-08-18",
            "--command",
            "cargo +nightly miri test read_header",
            "--limitation",
            "fixture only",
            "--out",
            "target/miri.json",
        ]))?;

        let Command::ReceiptImportMiri(options) = command else {
            return Err("expected receipt import-miri command".to_string());
        };
        assert_eq!(
            options.card_id,
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
        );
        assert_eq!(
            options.log,
            PathBuf::from("fixtures/raw_pointer_alignment_receipted/miri.success.log")
        );
        assert_eq!(options.author, "core/fixtures");
        assert_eq!(options.command, "cargo +nightly miri test read_header");
        assert_eq!(options.limitations, vec!["fixture only".to_string()]);
        Ok(())
    }

    #[test]
    fn parses_receipt_import_careful_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "import-careful",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--log",
            "fixtures/raw_pointer_alignment_receipted/careful.success.log",
            "--author",
            "core/fixtures",
            "--recorded-at",
            "2026-05-18T00:00:00Z",
            "--expires-at",
            "2026-08-18",
            "--command",
            "cargo +nightly careful test read_header",
            "--limitation",
            "fixture only",
            "--out",
            "target/careful.json",
        ]))?;

        let Command::ReceiptImportCareful(options) = command else {
            return Err("expected receipt import-careful command".to_string());
        };
        assert_eq!(
            options.card_id,
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
        );
        assert_eq!(
            options.log,
            PathBuf::from("fixtures/raw_pointer_alignment_receipted/careful.success.log")
        );
        assert_eq!(options.author, "core/fixtures");
        assert_eq!(options.command, "cargo +nightly careful test read_header");
        assert_eq!(options.limitations, vec!["fixture only".to_string()]);
        Ok(())
    }

    #[test]
    fn parses_receipt_import_cargo_careful_alias() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "import-cargo-careful",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--log=fixtures/raw_pointer_alignment_receipted/careful.success.log",
            "--author=core/fixtures",
            "--recorded-at=2026-05-18T00:00:00Z",
            "--expires-at=2026-08-18",
            "--command=cargo +nightly careful test read_header",
        ]))?;

        let Command::ReceiptImportCareful(options) = command else {
            return Err("expected receipt import-careful command".to_string());
        };
        assert_eq!(options.author, "core/fixtures");
        assert_eq!(options.command, "cargo +nightly careful test read_header");
        Ok(())
    }

    #[test]
    fn parses_receipt_import_sanitizer_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "import-sanitizer",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--tool",
            "asan",
            "--log",
            "fixtures/raw_pointer_alignment_receipted/asan.success.log",
            "--author",
            "core/fixtures",
            "--recorded-at",
            "2026-05-18T00:00:00Z",
            "--expires-at",
            "2026-08-18",
            "--command",
            "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header",
            "--limitation",
            "fixture only",
            "--out",
            "target/asan.json",
        ]))?;

        let Command::ReceiptImportSanitizer(options) = command else {
            return Err("expected receipt import-sanitizer command".to_string());
        };
        assert_eq!(
            options.card_id,
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
        );
        assert_eq!(options.tool.as_deref(), Some("asan"));
        assert_eq!(
            options.log,
            PathBuf::from("fixtures/raw_pointer_alignment_receipted/asan.success.log")
        );
        assert_eq!(options.author, "core/fixtures");
        assert_eq!(
            options.command,
            "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header"
        );
        assert_eq!(options.limitations, vec!["fixture only".to_string()]);
        Ok(())
    }

    #[test]
    fn parses_receipt_import_concurrency_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "import-concurrency",
            "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1",
            "--tool",
            "loom",
            "--log",
            "fixtures/unsafe_impl_send/loom.success.log",
            "--author",
            "core/fixtures",
            "--recorded-at",
            "2026-05-18T00:00:00Z",
            "--expires-at",
            "2026-08-18",
            "--command",
            "cargo test shared_cell_loom -- --nocapture",
            "--limitation",
            "fixture only",
            "--out",
            "target/loom.json",
        ]))?;

        let Command::ReceiptImportConcurrency(options) = command else {
            return Err("expected receipt import-concurrency command".to_string());
        };
        assert_eq!(
            options.card_id,
            "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1"
        );
        assert_eq!(options.tool.as_deref(), Some("loom"));
        assert_eq!(
            options.log,
            PathBuf::from("fixtures/unsafe_impl_send/loom.success.log")
        );
        assert_eq!(options.author, "core/fixtures");
        assert_eq!(
            options.command,
            "cargo test shared_cell_loom -- --nocapture"
        );
        assert_eq!(options.limitations, vec!["fixture only".to_string()]);
        Ok(())
    }

    #[test]
    fn parses_receipt_import_proof_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "import-proof",
            "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1",
            "--tool",
            "kani",
            "--log",
            "fixtures/transmute_invalid_value/kani.success.log",
            "--author",
            "core/fixtures",
            "--recorded-at",
            "2026-05-18T00:00:00Z",
            "--expires-at",
            "2026-08-18",
            "--command",
            "cargo kani --harness byte_to_bool_harness",
            "--limitation",
            "fixture only",
            "--out",
            "target/kani.json",
        ]))?;

        let Command::ReceiptImportProof(options) = command else {
            return Err("expected receipt import-proof command".to_string());
        };
        assert_eq!(
            options.card_id,
            "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1"
        );
        assert_eq!(options.tool.as_deref(), Some("kani"));
        assert_eq!(
            options.log,
            PathBuf::from("fixtures/transmute_invalid_value/kani.success.log")
        );
        assert_eq!(options.author, "core/fixtures");
        assert_eq!(options.command, "cargo kani --harness byte_to_bool_harness");
        assert_eq!(options.limitations, vec!["fixture only".to_string()]);
        Ok(())
    }

    #[test]
    fn receipt_import_miri_requires_command() {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "import-miri",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--log",
            "miri.log",
            "--author",
            "core/fixtures",
            "--recorded-at",
            "2026-05-18T00:00:00Z",
            "--expires-at",
            "2026-08-18",
        ]));

        assert_eq!(command, Err("missing value for --command".to_string()));
    }

    #[test]
    fn receipt_import_careful_requires_command() {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "import-careful",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--log",
            "careful.log",
            "--author",
            "core/fixtures",
            "--recorded-at",
            "2026-05-18T00:00:00Z",
            "--expires-at",
            "2026-08-18",
        ]));

        assert_eq!(command, Err("missing value for --command".to_string()));
    }

    #[test]
    fn receipt_import_sanitizer_requires_tool() {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "import-sanitizer",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--log",
            "asan.log",
            "--author",
            "core/fixtures",
            "--recorded-at",
            "2026-05-18T00:00:00Z",
            "--expires-at",
            "2026-08-18",
            "--command",
            "RUSTFLAGS='-Z sanitizer=address' cargo +nightly test read_header",
        ]));

        assert_eq!(command, Err("missing value for --tool".to_string()));
    }

    #[test]
    fn receipt_import_concurrency_requires_tool() {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "import-concurrency",
            "UR-unsafe-impl-send-src-lib-rs-sharedcell-unsafe_impl_send-unsafe_impl_send_sync-unsafe-impl-send-sync-e915d3491163-send_sync_invariant-c1",
            "--log",
            "loom.log",
            "--author",
            "core/fixtures",
            "--recorded-at",
            "2026-05-18T00:00:00Z",
            "--expires-at",
            "2026-08-18",
            "--command",
            "cargo test shared_cell_loom -- --nocapture",
        ]));

        assert_eq!(command, Err("missing value for --tool".to_string()));
    }

    #[test]
    fn receipt_import_proof_requires_tool() {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "import-proof",
            "UR-transmute-invalid-value-src-lib-rs-byte-to-bool-operation-transmute-u8-bool-bdefdb7b6120-invalid_value-c1",
            "--log",
            "kani.log",
            "--author",
            "core/fixtures",
            "--recorded-at",
            "2026-05-18T00:00:00Z",
            "--expires-at",
            "2026-08-18",
            "--command",
            "cargo kani --harness byte_to_bool_harness",
        ]));

        assert_eq!(command, Err("missing value for --tool".to_string()));
    }

    #[test]
    fn receipt_template_requires_metadata() {
        let command = parse(args([
            "unsafe-review",
            "receipt-template",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--tool",
            "miri",
        ]));

        assert_eq!(command, Err("missing value for --strength".to_string()));
    }

    #[test]
    fn parses_receipt_validate_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "validate",
            "--root=fixtures/raw_pointer_alignment_receipted",
        ]))?;

        assert_eq!(
            command,
            Command::ReceiptValidate {
                root: PathBuf::from("fixtures/raw_pointer_alignment_receipted"),
            }
        );
        Ok(())
    }

    #[test]
    fn parses_receipt_audit_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "audit",
            "--root=fixtures/raw_pointer_alignment_receipted",
            "--diff=change.diff",
            "--format=markdown",
            "--out=target/receipt-audit.md",
            "--max-cards=5",
        ]))?;

        let Command::ReceiptAudit(options) = command else {
            return Err("expected receipt audit command".to_string());
        };
        assert_eq!(
            options.root,
            PathBuf::from("fixtures/raw_pointer_alignment_receipted")
        );
        assert_eq!(
            options.diff,
            Some(DiffInput::File(PathBuf::from("change.diff")))
        );
        assert_eq!(options.format, Format::Markdown);
        assert_eq!(options.out, Some(PathBuf::from("target/receipt-audit.md")));
        assert_eq!(options.max_cards, Some(5));
        Ok(())
    }

    #[test]
    fn receipt_audit_rejects_non_audit_format() {
        let command = parse(args([
            "unsafe-review",
            "receipt",
            "audit",
            "--format=sarif",
        ]));

        assert_eq!(
            command,
            Err("receipt audit only supports json or markdown output, got `sarif`".to_string())
        );
    }

    #[test]
    fn parses_policy_report_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "policy",
            "report",
            "--root=fixtures/raw_pointer_alignment",
            "--diff=change.diff",
            "--format=markdown",
            "--out=target/policy-report.md",
            "--max-cards=5",
        ]))?;

        let Command::PolicyReport(options) = command else {
            return Err("expected policy report command".to_string());
        };
        assert_eq!(
            options.root,
            PathBuf::from("fixtures/raw_pointer_alignment")
        );
        assert_eq!(
            options.diff,
            Some(DiffInput::File(PathBuf::from("change.diff")))
        );
        assert_eq!(options.format, Format::Markdown);
        assert_eq!(options.out, Some(PathBuf::from("target/policy-report.md")));
        assert_eq!(options.max_cards, Some(5));
        Ok(())
    }

    #[test]
    fn policy_report_defaults_to_json_and_stays_advisory() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "policy", "report"]))?;

        let Command::PolicyReport(options) = command else {
            return Err("expected policy report command".to_string());
        };
        assert_eq!(options.format, Format::Json);
        assert_eq!(options.policy, PolicyMode::Advisory);
        Ok(())
    }

    #[test]
    fn policy_report_accepts_markdown_alias() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "policy", "report", "--format=md"]))?;

        let Command::PolicyReport(options) = command else {
            return Err("expected policy report command".to_string());
        };
        assert_eq!(options.format, Format::Markdown);
        Ok(())
    }

    #[test]
    fn policy_report_rejects_non_report_format() {
        let command = parse(args([
            "unsafe-review",
            "policy",
            "report",
            "--format=sarif",
        ]));

        assert_eq!(
            command,
            Err("policy report only supports json or markdown output, got `sarif`".to_string())
        );
    }

    #[test]
    fn policy_report_rejects_explicit_human_format() {
        let command = parse(args([
            "unsafe-review",
            "policy",
            "report",
            "--format=human",
        ]));

        assert_eq!(
            command,
            Err("policy report only supports json or markdown output, got `human`".to_string())
        );
    }

    #[test]
    fn policy_report_rejects_non_advisory_policy() {
        let command = parse(args([
            "unsafe-review",
            "policy",
            "report",
            "--policy=no-new-debt",
        ]));

        assert_eq!(command, Err("policy report is advisory-only".to_string()));
    }

    fn args<const N: usize>(values: [&str; N]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }
}
