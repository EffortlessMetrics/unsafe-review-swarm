use super::*;
use crate::command::Format;

mod arg_value;
mod saved_output;
mod template;

pub(super) use template::parse_receipt_template;

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
            saved_output::parse_saved_output_receipt(rest, "import-careful", false)
                .map(Command::ReceiptImportCareful)
        }
        "import-miri" => saved_output::parse_saved_output_receipt(rest, "import-miri", false)
            .map(Command::ReceiptImportMiri),
        "import-sanitizer" => {
            saved_output::parse_saved_output_receipt(rest, "import-sanitizer", true)
                .map(Command::ReceiptImportSanitizer)
        }
        "import-concurrency" => {
            saved_output::parse_saved_output_receipt(rest, "import-concurrency", true)
                .map(Command::ReceiptImportConcurrency)
        }
        "import-proof" => saved_output::parse_saved_output_receipt(rest, "import-proof", true)
            .map(Command::ReceiptImportProof),
        "template" => parse_receipt_template(rest).map(Command::ReceiptTemplate),
        "validate" => parse_receipt_validate(rest),
        "audit" => parse_receipt_audit(rest).map(Command::ReceiptAudit),
        other => Err(format!("unknown receipt subcommand `{other}`")),
    }
}

fn parse_receipt_audit(args: Vec<String>) -> Result<CheckOptions, String> {
    let explicit_format = has_explicit_format_arg(&args);
    let mut options = parse_check(args)?;
    options.format = if explicit_format && options.format == Format::Human {
        parse_receipt_audit_format(options.format)?
    } else {
        normalize_report_format(options.format, parse_receipt_audit_format)?
    };
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
