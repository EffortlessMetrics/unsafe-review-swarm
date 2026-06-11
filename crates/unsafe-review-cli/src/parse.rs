use crate::command::{
    BaselineAddOptions, BaselineCommand, BaselineInitOptions, CandidateCommand,
    CandidateImportOptions, CandidateLintOptions, CandidateListOptions, CandidateNewOptions,
    CandidateWitnessPlanOptions, CheckOptions, Command, ContextQuery, DiffInput, FirstPrOptions,
    Format, OutcomeOptions, RepoOptions,
};
use std::path::PathBuf;
use unsafe_review_core::{MANUAL_CANDIDATE_STABLE_BYTE_CLASSES, PolicyMode};

const DEFAULT_CANDIDATE_SKELETON_ID: &str = "R4R2-S000-TODO";

mod check_parse;
mod confirm;
mod policy;
mod receipt;

pub(crate) fn parse(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut rest = args.into_iter();
    let _binary = rest.next();
    let mut rest = rest.collect::<Vec<_>>();
    if rest.is_empty() {
        return Ok(Command::Help);
    }
    let command = rest.remove(0);
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        return Ok(Command::Help);
    }
    if command == "repo" && (has_help_flag(&rest) || is_exact_help_word(&rest)) {
        return Ok(Command::RepoHelp);
    }
    if command == "candidate"
        && (rest.is_empty() || has_help_flag(&rest) || is_candidate_help_word(&rest))
    {
        return Ok(Command::CandidateHelp);
    }
    if command == "baseline"
        && (rest.is_empty() || has_help_flag(&rest) || is_exact_help_word(&rest))
    {
        return Ok(Command::BaselineHelp);
    }
    if has_help_flag(&rest) {
        return Ok(Command::Help);
    }
    match command.as_str() {
        "--version" | "-V" => Ok(Command::Version),
        "support" => parse_support(rest),
        "doctor" => parse_doctor(rest),
        "check" => parse_check(rest).map(Command::Check),
        "first-pr" | "review" => parse_first_pr(rest).map(Command::FirstPr),
        "repo" => parse_repo(rest).map(Command::Repo),
        "pilot" => parse_check(rest).map(|mut options| {
            options.max_cards = Some(options.max_cards.unwrap_or(5));
            Command::Pilot(options)
        }),
        "badges" => parse_badges(rest),
        "explain" => parse_explain(rest),
        "context" => parse_context(rest),
        "candidate" => parse_candidate(rest).map(Command::Candidate),
        "baseline" => parse_baseline(rest).map(Command::Baseline),
        "confirm" => confirm::parse_confirm(rest).map(Command::Confirm),
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

fn parse_candidate(args: Vec<String>) -> Result<CandidateCommand, String> {
    let mut rest = args.into_iter();
    let Some(subcommand) = rest.next() else {
        return Err("missing candidate subcommand".to_string());
    };
    let rest = rest.collect::<Vec<_>>();
    match subcommand.as_str() {
        "new" => parse_candidate_new(rest).map(CandidateCommand::New),
        "import" => parse_candidate_import(rest).map(CandidateCommand::Import),
        "lint" => parse_candidate_lint(rest).map(CandidateCommand::Lint),
        "list" => parse_candidate_list(rest).map(CandidateCommand::List),
        "witness-plan" => parse_candidate_witness_plan(rest).map(CandidateCommand::WitnessPlan),
        other => Err(format!("unknown candidate subcommand `{other}`")),
    }
}

fn parse_candidate_new(args: Vec<String>) -> Result<CandidateNewOptions, String> {
    let mut class: Option<String> = None;
    let mut id = DEFAULT_CANDIDATE_SKELETON_ID.to_string();
    let mut out = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--class" => {
                idx += 1;
                class = Some(value(&args, idx, "--class")?.to_string());
            }
            arg if arg.starts_with("--class=") => {
                class = Some(inline_value(arg, "--class")?.to_string());
            }
            "--id" => {
                idx += 1;
                id = value(&args, idx, "--id")?.to_string();
            }
            arg if arg.starts_with("--id=") => {
                id = inline_value(arg, "--id")?.to_string();
            }
            "--out" => {
                idx += 1;
                out = Some(parse_path_value(&args, idx, "--out")?);
            }
            arg if arg.starts_with("--out=") => {
                out = Some(parse_inline_path_value(arg, "--out")?);
            }
            other => return Err(format!("unknown candidate new argument `{other}`")),
        }
        idx += 1;
    }
    let class = class.ok_or_else(|| {
        format!(
            "missing --class; valid stable-byte classes: {}",
            MANUAL_CANDIDATE_STABLE_BYTE_CLASSES.join(", ")
        )
    })?;
    if !MANUAL_CANDIDATE_STABLE_BYTE_CLASSES.contains(&class.as_str()) {
        return Err(format!(
            "unknown stable-byte class `{class}`; valid classes: {}",
            MANUAL_CANDIDATE_STABLE_BYTE_CLASSES.join(", ")
        ));
    }
    Ok(CandidateNewOptions { class, id, out })
}

fn parse_candidate_lint(args: Vec<String>) -> Result<CandidateLintOptions, String> {
    let mut input: Option<PathBuf> = None;
    for arg in &args {
        match arg.as_str() {
            value if value.starts_with('-') => {
                return Err(format!("unknown candidate lint argument `{value}`"));
            }
            value => {
                set_single_path(&mut input, value, "manual candidate input")?;
            }
        }
    }
    Ok(CandidateLintOptions {
        input: input.ok_or_else(|| "missing manual candidate input".to_string())?,
    })
}

fn parse_candidate_import(args: Vec<String>) -> Result<CandidateImportOptions, String> {
    let mut input: Option<PathBuf> = None;
    let mut out = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--out" => {
                idx += 1;
                out = Some(parse_path_value(&args, idx, "--out")?);
            }
            arg if arg.starts_with("--out=") => {
                out = Some(parse_inline_path_value(arg, "--out")?);
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown candidate import argument `{value}`"));
            }
            value => {
                set_single_path(&mut input, value, "manual candidate input")?;
            }
        }
        idx += 1;
    }
    Ok(CandidateImportOptions {
        input: input.ok_or_else(|| "missing manual candidate input".to_string())?,
        out,
    })
}

fn parse_candidate_list(args: Vec<String>) -> Result<CandidateListOptions, String> {
    let mut root = PathBuf::from(".");
    let mut format = Format::Markdown;
    let mut out = None;
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
                format = json_or_markdown_format(
                    parse_format(value(&args, idx, "--format")?)?,
                    "candidate list",
                )?;
            }
            arg if arg.starts_with("--format=") => {
                format = json_or_markdown_format(
                    parse_format(inline_value(arg, "--format")?)?,
                    "candidate list",
                )?;
            }
            "--out" => {
                idx += 1;
                out = Some(parse_path_value(&args, idx, "--out")?);
            }
            arg if arg.starts_with("--out=") => {
                out = Some(parse_inline_path_value(arg, "--out")?);
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown candidate list argument `{value}`"));
            }
            value => return Err(format!("unexpected candidate list argument `{value}`")),
        }
        idx += 1;
    }
    Ok(CandidateListOptions { root, format, out })
}

fn parse_candidate_witness_plan(args: Vec<String>) -> Result<CandidateWitnessPlanOptions, String> {
    let mut root = PathBuf::from(".");
    let mut id: Option<String> = None;
    let mut out = None;
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
                out = Some(parse_path_value(&args, idx, "--out")?);
            }
            arg if arg.starts_with("--out=") => {
                out = Some(parse_inline_path_value(arg, "--out")?);
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown candidate witness-plan argument `{value}`"));
            }
            value => set_card_id(&mut id, value)?,
        }
        idx += 1;
    }
    Ok(CandidateWitnessPlanOptions {
        root,
        id: id.ok_or_else(|| "missing manual candidate id".to_string())?,
        out,
    })
}

fn parse_baseline(args: Vec<String>) -> Result<BaselineCommand, String> {
    let mut rest = args.into_iter();
    let Some(subcommand) = rest.next() else {
        return Ok(BaselineCommand::Help);
    };
    let rest = rest.collect::<Vec<_>>();
    match subcommand.as_str() {
        "init" => parse_baseline_init(rest).map(BaselineCommand::Init),
        "add" => parse_baseline_add(rest).map(BaselineCommand::Add),
        other => Err(format!("unknown baseline subcommand `{other}`")),
    }
}

fn parse_baseline_init(args: Vec<String>) -> Result<BaselineInitOptions, String> {
    let mut options = BaselineInitOptions::default();
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--root" => {
                idx += 1;
                options.root = parse_path_value(&args, idx, "--root")?;
            }
            arg if arg.starts_with("--root=") => {
                options.root = parse_inline_path_value(arg, "--root")?;
            }
            "--out" => {
                idx += 1;
                options.out = Some(parse_path_value(&args, idx, "--out")?);
            }
            arg if arg.starts_with("--out=") => {
                options.out = Some(parse_inline_path_value(arg, "--out")?);
            }
            "--review-after" => {
                idx += 1;
                options.review_after = Some(parse_iso_date_value(&args, idx, "--review-after")?);
            }
            arg if arg.starts_with("--review-after=") => {
                options.review_after = Some(parse_inline_iso_date_value(arg, "--review-after")?);
            }
            other => return Err(format!("unknown baseline init argument `{other}`")),
        }
        idx += 1;
    }
    Ok(options)
}

fn parse_baseline_add(args: Vec<String>) -> Result<BaselineAddOptions, String> {
    let mut root = PathBuf::from(".");
    let mut card_id: Option<String> = None;
    let mut owner: Option<String> = None;
    let mut reason: Option<String> = None;
    let mut evidence: Option<String> = None;
    let mut review_after: Option<String> = None;
    let mut out: Option<PathBuf> = None;
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
            "--card-id" => {
                idx += 1;
                card_id = Some(value(&args, idx, "--card-id")?.to_string());
            }
            arg if arg.starts_with("--card-id=") => {
                card_id = Some(inline_value(arg, "--card-id")?.to_string());
            }
            "--owner" => {
                idx += 1;
                owner = Some(value(&args, idx, "--owner")?.to_string());
            }
            arg if arg.starts_with("--owner=") => {
                owner = Some(inline_value(arg, "--owner")?.to_string());
            }
            "--reason" => {
                idx += 1;
                reason = Some(value(&args, idx, "--reason")?.to_string());
            }
            arg if arg.starts_with("--reason=") => {
                reason = Some(inline_value(arg, "--reason")?.to_string());
            }
            "--evidence" => {
                idx += 1;
                evidence = Some(value(&args, idx, "--evidence")?.to_string());
            }
            arg if arg.starts_with("--evidence=") => {
                evidence = Some(inline_value(arg, "--evidence")?.to_string());
            }
            "--review-after" => {
                idx += 1;
                review_after = Some(parse_iso_date_value(&args, idx, "--review-after")?);
            }
            arg if arg.starts_with("--review-after=") => {
                review_after = Some(parse_inline_iso_date_value(arg, "--review-after")?);
            }
            "--out" => {
                idx += 1;
                out = Some(parse_path_value(&args, idx, "--out")?);
            }
            arg if arg.starts_with("--out=") => {
                out = Some(parse_inline_path_value(arg, "--out")?);
            }
            other => return Err(format!("unknown baseline add argument `{other}`")),
        }
        idx += 1;
    }
    Ok(BaselineAddOptions {
        root,
        card_id: card_id.ok_or("missing --card-id")?,
        owner: owner.ok_or("missing --owner")?,
        reason: reason.ok_or("missing --reason")?,
        evidence: evidence.ok_or("missing --evidence")?,
        review_after,
        out,
    })
}

fn parse_iso_date_value(args: &[String], idx: usize, flag: &str) -> Result<String, String> {
    let raw = value(args, idx, flag)?;
    validate_iso_date(raw, flag)?;
    Ok(raw.to_string())
}

fn parse_inline_iso_date_value(arg: &str, flag: &str) -> Result<String, String> {
    let raw = inline_value(arg, flag)?;
    validate_iso_date(raw, flag)?;
    Ok(raw.to_string())
}

fn validate_iso_date(raw: &str, flag: &str) -> Result<(), String> {
    let bytes = raw.as_bytes();
    if bytes.len() == 10
        && bytes[0..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
    {
        Ok(())
    } else {
        Err(format!("{flag} must be a YYYY-MM-DD date, got `{raw}`"))
    }
}

fn has_help_flag(args: &[String]) -> bool {
    args.iter()
        .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
}

fn is_exact_help_word(args: &[String]) -> bool {
    matches!(args, [arg] if arg == "help")
}

fn is_candidate_help_word(args: &[String]) -> bool {
    matches!(args, [arg] if arg == "help") || matches!(args, [_subcommand, arg] if arg == "help")
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
        // `--out` belongs to `check`/`repo`; `first-pr` uses `--out-dir`.
        // Intercept before try_apply_check_arg silently consumes it.
        let arg = args[idx].as_str();
        if arg == "--out" || arg.starts_with("--out=") {
            return Err(format!(
                "unknown first-pr argument `{arg}`; did you mean `--out-dir`?"
            ));
        }
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

fn parse_repo(args: Vec<String>) -> Result<RepoOptions, String> {
    let mut options = RepoOptions::default();
    let mut idx = 0usize;
    while idx < args.len() {
        if let Some(consumed) = check_parse::try_apply_check_arg(&args, idx, &mut options.check)? {
            idx += consumed;
            continue;
        }
        match args[idx].as_str() {
            "--include" => {
                idx += 1;
                options
                    .discovery
                    .include
                    .push(value(&args, idx, "--include")?.to_string());
            }
            arg if arg.starts_with("--include=") => {
                options
                    .discovery
                    .include
                    .push(inline_value(arg, "--include")?.to_string());
            }
            "--exclude" => {
                idx += 1;
                options
                    .discovery
                    .exclude
                    .push(value(&args, idx, "--exclude")?.to_string());
            }
            arg if arg.starts_with("--exclude=") => {
                options
                    .discovery
                    .exclude
                    .push(inline_value(arg, "--exclude")?.to_string());
            }
            "--list-files" | "--dry-run" => {
                options.list_files = true;
            }
            "--progress" => {
                options.progress = true;
            }
            "--timeout-seconds" => {
                idx += 1;
                options.timeout_seconds = Some(parse_timeout_seconds(value(
                    &args,
                    idx,
                    "--timeout-seconds",
                )?)?);
            }
            arg if arg.starts_with("--timeout-seconds=") => {
                options.timeout_seconds = Some(parse_timeout_seconds(inline_value(
                    arg,
                    "--timeout-seconds",
                )?)?);
            }
            "--respect-gitignore" => {
                options.discovery.respect_gitignore = true;
            }
            "--no-respect-gitignore" | "--no-gitignore" => {
                options.discovery.respect_gitignore = false;
            }
            "--large-repo-ignores" => {
                options.discovery.large_repo_ignores = true;
            }
            "--no-large-repo-ignores" => {
                options.discovery.large_repo_ignores = false;
            }
            "--max-files" => {
                idx += 1;
                options.discovery.max_files =
                    Some(parse_max_files(value(&args, idx, "--max-files")?)?);
            }
            arg if arg.starts_with("--max-files=") => {
                options.discovery.max_files =
                    Some(parse_max_files(inline_value(arg, "--max-files")?)?);
            }
            other => return Err(format!("unknown repo argument `{other}`")),
        }
        idx += 1;
    }
    validate_check_options(&options.check)?;
    Ok(options)
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
    let mut card_id: Option<String> = None;
    let mut file: Option<PathBuf> = None;
    let mut line_start: Option<u32> = None;
    let mut line_end: Option<u32> = None;
    let mut changed_only = false;
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
            "--file" => {
                idx += 1;
                let raw = value(&args, idx, "--file")?;
                file = Some(PathBuf::from(raw));
            }
            arg if arg.starts_with("--file=") => {
                file = Some(PathBuf::from(inline_value(arg, "--file")?));
            }
            "--lines" => {
                idx += 1;
                let raw = value(&args, idx, "--lines")?;
                let (s, e) = parse_line_range(raw)?;
                line_start = Some(s);
                line_end = Some(e);
            }
            arg if arg.starts_with("--lines=") => {
                let raw = inline_value(arg, "--lines")?;
                let (s, e) = parse_line_range(raw)?;
                line_start = Some(s);
                line_end = Some(e);
            }
            "--changed-only" => {
                changed_only = true;
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
            value => set_card_id(&mut card_id, value)?,
        }
        idx += 1;
    }

    // Determine the query mode: file-range or card-id.
    let has_file = file.is_some();
    let has_lines = line_start.is_some();
    let has_id = card_id.is_some();

    if has_file && has_id {
        return Err("context: use either a card-id or --file/--lines, not both".to_string());
    }
    if has_lines && !has_file {
        return Err("context: --lines requires --file".to_string());
    }
    if has_file && !has_lines {
        return Err("context: --file requires --lines Y-Z".to_string());
    }
    if changed_only && !has_file {
        return Err("context: --changed-only requires --file and --lines".to_string());
    }

    if let (Some(f), Some(s), Some(e)) = (file, line_start, line_end) {
        return Ok(Command::Context {
            root,
            query: ContextQuery::FileRange {
                file: f,
                line_start: s,
                line_end: e,
                changed_only,
            },
        });
    }

    match card_id {
        Some(id) => Ok(Command::Context {
            root,
            query: ContextQuery::CardId(id),
        }),
        None => Err(
            "missing card id (or use --file <path> --lines Y-Z for a file-range scan)".to_string(),
        ),
    }
}

fn parse_line_range(raw: &str) -> Result<(u32, u32), String> {
    let Some((left, right)) = raw.split_once('-') else {
        return Err(format!(
            "invalid --lines value `{raw}`: expected format Y-Z (e.g. 10-20)"
        ));
    };
    let start = left.parse::<u32>().map_err(|_parse_err| {
        format!("invalid --lines value `{raw}`: start `{left}` is not a positive integer")
    })?;
    let end = right.parse::<u32>().map_err(|_parse_err| {
        format!("invalid --lines value `{raw}`: end `{right}` is not a positive integer")
    })?;
    if start == 0 || end == 0 {
        return Err(format!(
            "invalid --lines value `{raw}`: line numbers must be >= 1"
        ));
    }
    if start > end {
        return Err(format!(
            "invalid --lines value `{raw}`: start {start} is after end {end}"
        ));
    }
    Ok((start, end))
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

fn set_single_path(path: &mut Option<PathBuf>, value: &str, name: &str) -> Result<(), String> {
    if path.replace(PathBuf::from(value)).is_some() {
        return Err(format!("expected exactly one {name}"));
    }
    Ok(())
}

fn parse_max_cards(raw: &str) -> Result<usize, String> {
    raw.parse::<usize>()
        .map_err(|err| format!("invalid --max-cards `{raw}`: {err}"))
}

fn parse_max_files(raw: &str) -> Result<usize, String> {
    raw.parse::<usize>()
        .map_err(|err| format!("invalid --max-files `{raw}`: {err}"))
}

fn parse_timeout_seconds(raw: &str) -> Result<u64, String> {
    let seconds = raw
        .parse::<u64>()
        .map_err(|err| format!("invalid --timeout-seconds `{raw}`: {err}"))?;
    if seconds == 0 {
        return Err("invalid --timeout-seconds `0`: value must be greater than 0".to_string());
    }
    Ok(seconds)
}

fn parse_format(raw: &str) -> Result<Format, String> {
    match raw {
        "human" => Ok(Format::Human),
        "json" | "repo-json" => Ok(Format::Json),
        "markdown" | "md" => Ok(Format::Markdown),
        "pr-summary" => Ok(Format::PrSummary),
        "github-summary" | "github-markdown" => Ok(Format::GithubSummary),
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
        Format::GithubSummary => "github-summary",
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
    fn parses_repo_help_as_repo_specific_help() -> Result<(), String> {
        assert_eq!(
            parse(args(["unsafe-review", "repo", "--help"]))?,
            Command::RepoHelp
        );
        assert_eq!(
            parse(args(["unsafe-review", "repo", "-h"]))?,
            Command::RepoHelp
        );
        assert_eq!(
            parse(args(["unsafe-review", "repo", "help"]))?,
            Command::RepoHelp
        );
        Ok(())
    }

    #[test]
    fn parses_candidate_help_as_candidate_specific_help() -> Result<(), String> {
        assert_eq!(
            parse(args(["unsafe-review", "candidate"]))?,
            Command::CandidateHelp
        );
        assert_eq!(
            parse(args(["unsafe-review", "candidate", "--help"]))?,
            Command::CandidateHelp
        );
        assert_eq!(
            parse(args(["unsafe-review", "candidate", "-h"]))?,
            Command::CandidateHelp
        );
        assert_eq!(
            parse(args(["unsafe-review", "candidate", "help"]))?,
            Command::CandidateHelp
        );
        assert_eq!(
            parse(args(["unsafe-review", "candidate", "import", "--help"]))?,
            Command::CandidateHelp
        );
        assert_eq!(
            parse(args(["unsafe-review", "candidate", "witness-plan", "help"]))?,
            Command::CandidateHelp
        );
        assert_eq!(
            parse(args([
                "unsafe-review",
                "candidate",
                "import",
                "candidate.json",
                "--out",
                "help"
            ]))?,
            Command::Candidate(CandidateCommand::Import(CandidateImportOptions {
                input: PathBuf::from("candidate.json"),
                out: Some(PathBuf::from("help")),
            }))
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
        assert_eq!(options.format, Format::GithubSummary);
        Ok(())
    }

    #[test]
    fn parses_candidate_new_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "candidate",
            "new",
            "--class",
            "stable-byte-source-getter-reentry",
            "--id=R4R2-S010",
            "--out",
            "target/draft-candidate.json",
        ]))?;

        let Command::Candidate(CandidateCommand::New(options)) = command else {
            return Err("expected candidate new command".to_string());
        };
        assert_eq!(options.class, "stable-byte-source-getter-reentry");
        assert_eq!(options.id, "R4R2-S010");
        assert_eq!(
            options.out,
            Some(PathBuf::from("target/draft-candidate.json"))
        );
        Ok(())
    }

    #[test]
    fn candidate_new_defaults_skeleton_id() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "candidate",
            "new",
            "--class=stable-byte-source-sab-race",
        ]))?;

        let Command::Candidate(CandidateCommand::New(options)) = command else {
            return Err("expected candidate new command".to_string());
        };
        assert_eq!(options.class, "stable-byte-source-sab-race");
        assert_eq!(options.id, "R4R2-S000-TODO");
        assert_eq!(options.out, None);
        Ok(())
    }

    #[test]
    fn candidate_new_rejects_unknown_class_listing_valid_classes() {
        let command = parse(args([
            "unsafe-review",
            "candidate",
            "new",
            "--class",
            "stable-byte-source-unknown",
        ]));

        let err = command.err().unwrap_or_default();
        assert!(
            err.contains("unknown stable-byte class `stable-byte-source-unknown`"),
            "{err}"
        );
        for class in MANUAL_CANDIDATE_STABLE_BYTE_CLASSES {
            assert!(err.contains(class), "{err} missing {class}");
        }
    }

    #[test]
    fn candidate_new_requires_class() {
        let command = parse(args(["unsafe-review", "candidate", "new"]));

        let err = command.err().unwrap_or_default();
        assert!(err.contains("missing --class"), "{err}");
        assert!(err.contains("stable-byte-source-getter-reentry"), "{err}");
    }

    #[test]
    fn candidate_new_rejects_unknown_arguments() {
        let command = parse(args([
            "unsafe-review",
            "candidate",
            "new",
            "--class=stable-byte-source-sab-race",
            "--format=json",
        ]));

        assert_eq!(
            command,
            Err("unknown candidate new argument `--format=json`".to_string())
        );
    }

    #[test]
    fn parses_candidate_lint_command() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "candidate", "lint", "draft.json"]))?;

        assert_eq!(
            command,
            Command::Candidate(CandidateCommand::Lint(CandidateLintOptions {
                input: PathBuf::from("draft.json"),
            }))
        );
        Ok(())
    }

    #[test]
    fn candidate_lint_requires_input_file() {
        let command = parse(args(["unsafe-review", "candidate", "lint"]));

        assert_eq!(command, Err("missing manual candidate input".to_string()));
    }

    #[test]
    fn candidate_lint_rejects_extra_inputs_and_flags() {
        let extra = parse(args([
            "unsafe-review",
            "candidate",
            "lint",
            "draft.json",
            "other.json",
        ]));
        let flag = parse(args([
            "unsafe-review",
            "candidate",
            "lint",
            "--out",
            "draft.json",
        ]));

        assert_eq!(
            extra,
            Err("expected exactly one manual candidate input".to_string())
        );
        assert_eq!(
            flag,
            Err("unknown candidate lint argument `--out`".to_string())
        );
    }

    #[test]
    fn parses_candidate_import_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "candidate",
            "import",
            "candidate.json",
            "--out",
            ".unsafe-review/candidates/R4R2-S001.json",
        ]))?;

        let Command::Candidate(CandidateCommand::Import(options)) = command else {
            return Err("expected candidate import command".to_string());
        };
        assert_eq!(options.input, PathBuf::from("candidate.json"));
        assert_eq!(
            options.out,
            Some(PathBuf::from(".unsafe-review/candidates/R4R2-S001.json"))
        );
        Ok(())
    }

    #[test]
    fn parses_candidate_list_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "candidate",
            "list",
            "--root",
            ".",
            "--format=json",
            "--out",
            "target/manual-candidates.json",
        ]))?;

        let Command::Candidate(CandidateCommand::List(options)) = command else {
            return Err("expected candidate list command".to_string());
        };
        assert_eq!(options.root, PathBuf::from("."));
        assert_eq!(options.format, Format::Json);
        assert_eq!(
            options.out,
            Some(PathBuf::from("target/manual-candidates.json"))
        );
        Ok(())
    }

    #[test]
    fn parses_candidate_witness_plan_command() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "candidate",
            "witness-plan",
            "--root",
            ".",
            "R4R2-S001",
            "--out=target/manual-witness-plan.md",
        ]))?;

        let Command::Candidate(CandidateCommand::WitnessPlan(options)) = command else {
            return Err("expected candidate witness-plan command".to_string());
        };
        assert_eq!(options.root, PathBuf::from("."));
        assert_eq!(options.id, "R4R2-S001");
        assert_eq!(
            options.out,
            Some(PathBuf::from("target/manual-witness-plan.md"))
        );
        Ok(())
    }

    #[test]
    fn parses_repo_file_selection_options() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "repo",
            "--root=.",
            "--include",
            "src/**/*.rs",
            "--include=packages/**/*.rs",
            "--exclude",
            "vendor/**",
            "--exclude=**/generated/**",
            "--list-files",
            "--max-files=25",
            "--timeout-seconds",
            "30",
            "--no-respect-gitignore",
        ]))?;

        let Command::Repo(options) = command else {
            return Err("expected repo command".to_string());
        };
        assert_eq!(options.check.root, PathBuf::from("."));
        assert_eq!(
            options.discovery.include,
            vec!["src/**/*.rs".to_string(), "packages/**/*.rs".to_string()]
        );
        assert_eq!(
            options.discovery.exclude,
            vec!["vendor/**".to_string(), "**/generated/**".to_string()]
        );
        assert_eq!(options.discovery.max_files, Some(25));
        assert_eq!(options.timeout_seconds, Some(30));
        assert!(!options.discovery.respect_gitignore);
        assert!(options.list_files);
        assert!(!options.progress);
        Ok(())
    }

    #[test]
    fn parses_repo_progress_option() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "repo", "--progress"]))?;

        let Command::Repo(options) = command else {
            return Err("expected repo command".to_string());
        };
        assert!(options.progress);
        Ok(())
    }

    #[test]
    fn repo_dry_run_is_alias_for_list_files() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "repo", "--dry-run"]))?;

        let Command::Repo(options) = command else {
            return Err("expected repo command".to_string());
        };
        assert!(options.list_files);
        Ok(())
    }

    #[test]
    fn repo_dry_run_combined_with_other_flags() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "repo",
            "--dry-run",
            "--include=src/**/*.rs",
            "--max-files=100",
        ]))?;

        let Command::Repo(options) = command else {
            return Err("expected repo command".to_string());
        };
        assert!(options.list_files);
        assert_eq!(options.discovery.include, vec!["src/**/*.rs".to_string()]);
        assert_eq!(options.discovery.max_files, Some(100));
        Ok(())
    }

    #[test]
    fn repo_large_repo_ignores_flag() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "repo", "--large-repo-ignores"]))?;

        let Command::Repo(options) = command else {
            return Err("expected repo command".to_string());
        };
        assert!(options.discovery.large_repo_ignores);
        Ok(())
    }

    #[test]
    fn repo_no_large_repo_ignores_flag() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "repo", "--no-large-repo-ignores"]))?;

        let Command::Repo(options) = command else {
            return Err("expected repo command".to_string());
        };
        assert!(!options.discovery.large_repo_ignores);
        Ok(())
    }

    #[test]
    fn repo_large_repo_ignores_default_is_true() -> Result<(), String> {
        let command = parse(args(["unsafe-review", "repo"]))?;

        let Command::Repo(options) = command else {
            return Err("expected repo command".to_string());
        };
        assert!(
            options.discovery.large_repo_ignores,
            "large_repo_ignores must default to true to preserve existing behavior"
        );
        Ok(())
    }

    #[test]
    fn repo_rejects_zero_timeout() {
        let command = parse(args(["unsafe-review", "repo", "--timeout-seconds=0"]));

        assert_eq!(
            command,
            Err("invalid --timeout-seconds `0`: value must be greater than 0".to_string())
        );
    }

    #[test]
    fn check_rejects_repo_only_file_selection_options() {
        let command = parse(args(["unsafe-review", "check", "--include", "src/**/*.rs"]));

        assert_eq!(command, Err("unknown argument `--include`".to_string()));
    }

    #[test]
    fn check_rejects_repo_only_progress_option() {
        let command = parse(args(["unsafe-review", "check", "--progress"]));

        assert_eq!(command, Err("unknown argument `--progress`".to_string()));
    }

    #[test]
    fn check_rejects_repo_only_timeout_option() {
        let command = parse(args(["unsafe-review", "check", "--timeout-seconds=10"]));

        assert_eq!(
            command,
            Err("unknown argument `--timeout-seconds=10`".to_string())
        );
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
    fn first_pr_rejects_out_and_suggests_out_dir() {
        // Regression test for EffortlessMetrics/unsafe-review#531: `--out` is
        // a `check`/`repo` flag; `first-pr` uses `--out-dir`. The parser must
        // reject `--out` with a suggestion rather than silently consuming it.
        let space_form = parse(args([
            "unsafe-review",
            "first-pr",
            "--diff=change.diff",
            "--out",
            "/tmp/sensor-dir",
        ]));
        let equals_form = parse(args([
            "unsafe-review",
            "first-pr",
            "--diff=change.diff",
            "--out=/tmp/sensor-dir",
        ]));

        for result in [space_form, equals_form] {
            let err = result.err().unwrap_or_default();
            assert!(
                err.contains("--out"),
                "error must name the unknown flag: {err}"
            );
            assert!(
                err.contains("--out-dir"),
                "error must suggest --out-dir: {err}"
            );
        }
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
                query: ContextQuery::CardId("UR-card".to_string()),
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
                query: ContextQuery::CardId("UR-card".to_string()),
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
    fn parses_context_file_range() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "context",
            "--file",
            "src/lib.rs",
            "--lines",
            "10-20",
            "--json",
        ]))?;
        assert_eq!(
            command,
            Command::Context {
                root: PathBuf::from("."),
                query: ContextQuery::FileRange {
                    file: PathBuf::from("src/lib.rs"),
                    line_start: 10,
                    line_end: 20,
                    changed_only: false,
                },
            }
        );
        Ok(())
    }

    #[test]
    fn parses_context_file_range_changed_only() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "context",
            "--root",
            "fixtures/raw_pointer_alignment",
            "--file=src/lib.rs",
            "--lines=5-15",
            "--changed-only",
            "--json",
        ]))?;
        assert_eq!(
            command,
            Command::Context {
                root: PathBuf::from("fixtures/raw_pointer_alignment"),
                query: ContextQuery::FileRange {
                    file: PathBuf::from("src/lib.rs"),
                    line_start: 5,
                    line_end: 15,
                    changed_only: true,
                },
            }
        );
        Ok(())
    }

    #[test]
    fn rejects_context_file_without_lines() {
        let command = parse(args(["unsafe-review", "context", "--file", "src/lib.rs"]));
        assert_eq!(
            command,
            Err("context: --file requires --lines Y-Z".to_string())
        );
    }

    #[test]
    fn rejects_context_lines_without_file() {
        let command = parse(args(["unsafe-review", "context", "--lines", "10-20"]));
        assert_eq!(command, Err("context: --lines requires --file".to_string()));
    }

    #[test]
    fn rejects_context_file_and_card_id_together() {
        let command = parse(args([
            "unsafe-review",
            "context",
            "--file",
            "src/lib.rs",
            "--lines",
            "10-20",
            "UR-card",
        ]));
        assert_eq!(
            command,
            Err("context: use either a card-id or --file/--lines, not both".to_string())
        );
    }

    #[test]
    fn rejects_context_changed_only_without_file() {
        let command = parse(args([
            "unsafe-review",
            "context",
            "--changed-only",
            "UR-card",
        ]));
        assert_eq!(
            command,
            Err("context: --changed-only requires --file and --lines".to_string())
        );
    }

    #[test]
    fn rejects_invalid_line_range_formats() {
        let bad_range = parse(args([
            "unsafe-review",
            "context",
            "--file",
            "src/lib.rs",
            "--lines",
            "abc-20",
        ]));
        assert!(
            bad_range
                .err()
                .unwrap_or_default()
                .contains("not a positive integer"),
            "expected parse error for non-numeric start"
        );

        let reversed = parse(args([
            "unsafe-review",
            "context",
            "--file",
            "src/lib.rs",
            "--lines",
            "20-10",
        ]));
        assert_eq!(
            reversed,
            Err("invalid --lines value `20-10`: start 20 is after end 10".to_string())
        );
    }

    #[test]
    fn parses_confirm_command_with_allow_heavy_opt_in() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "confirm",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--allow-heavy",
            "--root=fixtures/raw_pointer_alignment",
            "--author",
            "core/fixtures",
            "--expires-at=2026-09-18",
            "--timeout-seconds=120",
            "--command",
            "cargo +nightly miri test read_header",
            "--out",
            "target/confirm-receipt.json",
        ]))?;

        let Command::Confirm(options) = command else {
            return Err("expected confirm command".to_string());
        };
        assert_eq!(
            options.card_id,
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1"
        );
        assert_eq!(
            options.root,
            PathBuf::from("fixtures/raw_pointer_alignment")
        );
        assert_eq!(options.author, "core/fixtures");
        assert_eq!(options.expires_at.as_deref(), Some("2026-09-18"));
        assert_eq!(options.timeout_seconds, 120);
        assert_eq!(
            options.command.as_deref(),
            Some("cargo +nightly miri test read_header")
        );
        assert_eq!(
            options.out,
            Some(PathBuf::from("target/confirm-receipt.json"))
        );
        Ok(())
    }

    #[test]
    fn confirm_defaults_timeout_to_600_seconds() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "confirm",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--allow-heavy",
            "--author=core/fixtures",
        ]))?;

        let Command::Confirm(options) = command else {
            return Err("expected confirm command".to_string());
        };
        assert_eq!(options.timeout_seconds, 600);
        assert_eq!(options.expires_at, None);
        assert_eq!(options.command, None);
        assert_eq!(options.out, None);
        Ok(())
    }

    #[test]
    fn confirm_refuses_without_allow_heavy_opt_in() {
        let command = parse(args([
            "unsafe-review",
            "confirm",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--author",
            "core/fixtures",
        ]));

        let err = command.err().unwrap_or_default();
        assert!(err.contains("only with the explicit --allow-heavy opt-in"));
        assert!(err.contains("unsafe-review never executes witnesses by default"));
        assert!(err.contains("--dry-run to preview"));
    }

    #[test]
    fn confirm_dry_run_needs_neither_allow_heavy_nor_author() -> Result<(), String> {
        let command = parse(args([
            "unsafe-review",
            "confirm",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--dry-run",
            "--root=fixtures/raw_pointer_alignment",
        ]))?;

        let Command::Confirm(options) = command else {
            return Err("expected confirm command".to_string());
        };
        assert!(options.dry_run);
        assert_eq!(options.author, "");
        assert_eq!(options.timeout_seconds, 600);
        Ok(())
    }

    #[test]
    fn confirm_rejects_dry_run_combined_with_allow_heavy() {
        let command = parse(args([
            "unsafe-review",
            "confirm",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--dry-run",
            "--allow-heavy",
            "--author=core/fixtures",
        ]));

        let err = command.err().unwrap_or_default();
        assert!(err.contains("choose only one of --dry-run or --allow-heavy"));
    }

    #[test]
    fn confirm_requires_author_for_accountability() {
        let command = parse(args([
            "unsafe-review",
            "confirm",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--allow-heavy",
        ]));

        let err = command.err().unwrap_or_default();
        assert!(err.contains("confirm requires --author"));
        assert!(err.contains("accountability"));
    }

    #[test]
    fn confirm_rejects_conflicting_diff_sources() {
        let command = parse(args([
            "unsafe-review",
            "confirm",
            "UR-crate-src-lib-rs-owner-operation-raw_pointer_read-read-deadbeef1234-alignment-c1",
            "--allow-heavy",
            "--author=core/fixtures",
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
