use super::*;
use crate::command::{ReceiptTemplateOptions, SavedOutputReceiptOptions};

pub(super) fn parse_receipt(args: Vec<String>) -> Result<Command, String> {
    let mut rest = args;
    let Some(subcommand) = rest.first() else {
        return Err(
            "missing receipt subcommand `import-miri`, `import-careful`, `import-sanitizer`, `import-concurrency`, `import-proof`, `template`, `validate`, or `audit`"
                .to_string(),
        );
    };
    let subcommand = subcommand.clone();
    rest.remove(0);
    match subcommand.as_str() {
        "import-careful" | "import-cargo-careful" => {
            parse_saved_output_receipt(rest, "import-careful", false)
                .map(Command::ReceiptImportCareful)
        }
        "import-miri" => {
            parse_saved_output_receipt(rest, "import-miri", false).map(Command::ReceiptImportMiri)
        }
        "import-sanitizer" => parse_saved_output_receipt(rest, "import-sanitizer", true)
            .map(Command::ReceiptImportSanitizer),
        "import-concurrency" => parse_saved_output_receipt(rest, "import-concurrency", true)
            .map(Command::ReceiptImportConcurrency),
        "import-proof" => {
            parse_saved_output_receipt(rest, "import-proof", true).map(Command::ReceiptImportProof)
        }
        "template" => parse_receipt_template(rest).map(Command::ReceiptTemplate),
        "validate" => parse_receipt_validate(rest),
        "audit" => parse_receipt_audit(rest).map(Command::ReceiptAudit),
        other => Err(format!("unknown receipt subcommand `{other}`")),
    }
}

pub(super) fn parse_receipt_template(args: Vec<String>) -> Result<ReceiptTemplateOptions, String> {
    let mut options = ReceiptTemplateOptions::default();
    let mut id: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--tool" => {
                idx += 1;
                options.tool = value(&args, idx, "--tool")?.to_string();
            }
            arg if arg.starts_with("--tool=") => {
                options.tool = inline_value(arg, "--tool")?.to_string();
            }
            "--strength" => {
                idx += 1;
                options.strength = value(&args, idx, "--strength")?.to_string();
            }
            arg if arg.starts_with("--strength=") => {
                options.strength = inline_value(arg, "--strength")?.to_string();
            }
            "--author" => {
                idx += 1;
                options.author = value(&args, idx, "--author")?.to_string();
            }
            arg if arg.starts_with("--author=") => {
                options.author = inline_value(arg, "--author")?.to_string();
            }
            "--recorded-at" => {
                idx += 1;
                options.recorded_at = value(&args, idx, "--recorded-at")?.to_string();
            }
            arg if arg.starts_with("--recorded-at=") => {
                options.recorded_at = inline_value(arg, "--recorded-at")?.to_string();
            }
            "--expires-at" => {
                idx += 1;
                options.expires_at = value(&args, idx, "--expires-at")?.to_string();
            }
            arg if arg.starts_with("--expires-at=") => {
                options.expires_at = inline_value(arg, "--expires-at")?.to_string();
            }
            "--summary" => {
                idx += 1;
                options.summary = Some(value(&args, idx, "--summary")?.to_string());
            }
            arg if arg.starts_with("--summary=") => {
                options.summary = Some(inline_value(arg, "--summary")?.to_string());
            }
            "--command" => {
                idx += 1;
                options.command = Some(value(&args, idx, "--command")?.to_string());
            }
            arg if arg.starts_with("--command=") => {
                options.command = Some(inline_value(arg, "--command")?.to_string());
            }
            "--limitation" => {
                idx += 1;
                options
                    .limitations
                    .push(value(&args, idx, "--limitation")?.to_string());
            }
            arg if arg.starts_with("--limitation=") => {
                options
                    .limitations
                    .push(inline_value(arg, "--limitation")?.to_string());
            }
            "--out" => {
                idx += 1;
                options.out = Some(PathBuf::from(value(&args, idx, "--out")?));
            }
            arg if arg.starts_with("--out=") => {
                options.out = Some(PathBuf::from(inline_value(arg, "--out")?));
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown receipt template argument `{value}`"));
            }
            value => set_card_id(&mut id, value)?,
        }
        idx += 1;
    }
    options.card_id = id.ok_or_else(|| "missing card id".to_string())?;
    validate_required_cli_value(&options.tool, "--tool")?;
    validate_required_cli_value(&options.strength, "--strength")?;
    validate_required_cli_value(&options.author, "--author")?;
    validate_required_cli_value(&options.recorded_at, "--recorded-at")?;
    validate_required_cli_value(&options.expires_at, "--expires-at")?;
    Ok(options)
}

fn parse_receipt_audit(args: Vec<String>) -> Result<CheckOptions, String> {
    let mut options = parse_check(args)?;
    options.format = normalize_report_format(options.format, parse_receipt_audit_format)?;
    require_advisory_policy(&options, "receipt audit is advisory-only")?;
    Ok(options)
}

fn parse_receipt_validate(args: Vec<String>) -> Result<Command, String> {
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
            other => return Err(format!("unknown receipt validate argument `{other}`")),
        }
        idx += 1;
    }
    Ok(Command::ReceiptValidate { root })
}

fn parse_saved_output_receipt(
    args: Vec<String>,
    command_name: &str,
    allow_tool: bool,
) -> Result<SavedOutputReceiptOptions, String> {
    let mut options = SavedOutputReceiptOptions::default();
    let mut id: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        let arg = args[idx].as_str();
        if parse_saved_output_receipt_tool(&args, &mut idx, arg, &mut options, allow_tool)? {
            idx += 1;
            continue;
        }
        if parse_saved_output_receipt_common(&args, &mut idx, arg, &mut options)? {
            idx += 1;
            continue;
        }
        match arg {
            value if value.starts_with('-') => {
                return Err(format!("unknown receipt {command_name} argument `{value}`"));
            }
            value => set_card_id(&mut id, value)?,
        }
        idx += 1;
    }
    options.card_id = id.ok_or_else(|| "missing card id".to_string())?;
    if options.log.as_os_str().is_empty() {
        return Err("missing value for --log".to_string());
    }
    validate_required_cli_value(&options.author, "--author")?;
    validate_required_cli_value(&options.recorded_at, "--recorded-at")?;
    validate_required_cli_value(&options.expires_at, "--expires-at")?;
    validate_required_cli_value(&options.command, "--command")?;
    if allow_tool && options.tool.as_deref().unwrap_or("").trim().is_empty() {
        return Err("missing value for --tool".to_string());
    }
    Ok(options)
}

fn parse_saved_output_receipt_tool(
    args: &[String],
    idx: &mut usize,
    arg: &str,
    options: &mut SavedOutputReceiptOptions,
    allow_tool: bool,
) -> Result<bool, String> {
    match arg {
        "--tool" if allow_tool => {
            *idx += 1;
            options.tool = Some(value(args, *idx, "--tool")?.to_string());
            Ok(true)
        }
        _ if allow_tool && arg.starts_with("--tool=") => {
            options.tool = Some(inline_value(arg, "--tool")?.to_string());
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn parse_saved_output_receipt_common(
    args: &[String],
    idx: &mut usize,
    arg: &str,
    options: &mut SavedOutputReceiptOptions,
) -> Result<bool, String> {
    match arg {
        "--log" => {
            *idx += 1;
            options.log = PathBuf::from(value(args, *idx, "--log")?);
            Ok(true)
        }
        _ if arg.starts_with("--log=") => {
            options.log = PathBuf::from(inline_value(arg, "--log")?);
            Ok(true)
        }
        "--author" => {
            *idx += 1;
            options.author = value(args, *idx, "--author")?.to_string();
            Ok(true)
        }
        _ if arg.starts_with("--author=") => {
            options.author = inline_value(arg, "--author")?.to_string();
            Ok(true)
        }
        "--recorded-at" => {
            *idx += 1;
            options.recorded_at = value(args, *idx, "--recorded-at")?.to_string();
            Ok(true)
        }
        _ if arg.starts_with("--recorded-at=") => {
            options.recorded_at = inline_value(arg, "--recorded-at")?.to_string();
            Ok(true)
        }
        "--expires-at" => {
            *idx += 1;
            options.expires_at = value(args, *idx, "--expires-at")?.to_string();
            Ok(true)
        }
        _ if arg.starts_with("--expires-at=") => {
            options.expires_at = inline_value(arg, "--expires-at")?.to_string();
            Ok(true)
        }
        "--command" => {
            *idx += 1;
            options.command = value(args, *idx, "--command")?.to_string();
            Ok(true)
        }
        _ if arg.starts_with("--command=") => {
            options.command = inline_value(arg, "--command")?.to_string();
            Ok(true)
        }
        "--limitation" => {
            *idx += 1;
            options
                .limitations
                .push(value(args, *idx, "--limitation")?.to_string());
            Ok(true)
        }
        _ if arg.starts_with("--limitation=") => {
            options
                .limitations
                .push(inline_value(arg, "--limitation")?.to_string());
            Ok(true)
        }
        "--out" => {
            *idx += 1;
            options.out = Some(PathBuf::from(value(args, *idx, "--out")?));
            Ok(true)
        }
        _ if arg.starts_with("--out=") => {
            options.out = Some(PathBuf::from(inline_value(arg, "--out")?));
            Ok(true)
        }
        _ => Ok(false),
    }
}
