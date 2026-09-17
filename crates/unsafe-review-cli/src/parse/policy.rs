use crate::command::{CheckOptions, Command, Format};

pub(super) fn parse_policy_command(args: Vec<String>) -> Result<Command, String> {
    let Some((subcommand, rest)) = args.split_first() else {
        return Err("missing policy subcommand `report`".to_string());
    };
    match subcommand.as_str() {
        "report" => parse_policy_report(rest).map(Command::PolicyReport),
        other => Err(format!("unknown policy subcommand `{other}`")),
    }
}

fn parse_policy_report(args: &[String]) -> Result<CheckOptions, String> {
    let explicit_format = super::has_explicit_format_arg(args);
    let mut options = super::parse_check(args.to_vec())?;
    // Follow-up review on #2245: `policy report` normalizes the shared
    // parser's human default into a report format with no short rendering.
    // Reject the flag here so it cannot ride along silently; `pilot` keeps
    // working because it renders through the shared check renderer.
    if options.short {
        return Err(
            "--short applies to check/repo human output; policy report does not support it"
                .to_string(),
        );
    }
    options.format = if explicit_format && options.format == Format::Human {
        normalize_policy_report_format(options.format)?
    } else {
        super::normalize_report_format(options.format, normalize_policy_report_format)?
    };
    super::require_advisory_policy(&options, "policy report is advisory-only")?;
    Ok(options)
}

fn normalize_policy_report_format(format: Format) -> Result<Format, String> {
    super::json_or_markdown_format(format, "policy report")
}
