use crate::command::{CheckOptions, Command, Format};
use unsafe_review_core::PolicyMode;

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
    let mut options = super::parse_check(args.to_vec())?;
    if !matches!(options.format, Format::Human) {
        options.format = parse_policy_report_format(super::format_name(&options.format))?;
    } else {
        options.format = Format::Json;
    }
    if options.policy != PolicyMode::Advisory {
        return Err("policy report is advisory-only".to_string());
    }
    Ok(options)
}

fn parse_policy_report_format(raw: &str) -> Result<Format, String> {
    match super::parse_format(raw)? {
        Format::Json => Ok(Format::Json),
        Format::Markdown => Ok(Format::Markdown),
        other => Err(format!(
            "policy report only supports json or markdown output, got `{}`",
            super::format_name(&other)
        )),
    }
}
