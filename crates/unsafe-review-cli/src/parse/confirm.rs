use super::*;
use crate::command::ConfirmOptions;

pub(super) const CONFIRM_ALLOW_HEAVY_REFUSAL: &str = "confirm executes the card's routed witness command only with the explicit --allow-heavy opt-in; unsafe-review never executes witnesses by default. Use --dry-run to preview the confirmation step without executing, or re-run with --allow-heavy to execute it, or run the routed witness yourself and import the saved output with `receipt import-*`";

pub(super) const CONFIRM_MODE_CONFLICT: &str = "choose only one of --dry-run or --allow-heavy: --dry-run previews the confirmation step without executing; --allow-heavy executes it";

pub(super) const CONFIRM_AUTHOR_REQUIRED: &str = "confirm requires --author <owner>: witness receipts demand accountability for who ran the confirmation";

pub(super) fn parse_confirm(args: Vec<String>) -> Result<ConfirmOptions, String> {
    let mut options = ConfirmOptions::default();
    let mut allow_heavy = false;
    let mut card_id: Option<String> = None;
    let mut author: Option<String> = None;
    let mut idx = 0usize;
    while idx < args.len() {
        match args[idx].as_str() {
            "--allow-heavy" => allow_heavy = true,
            "--dry-run" => options.dry_run = true,
            "--root" => {
                idx += 1;
                options.root = parse_path_value(&args, idx, "--root")?;
            }
            arg if arg.starts_with("--root=") => {
                options.root = parse_inline_path_value(arg, "--root")?;
            }
            "--base" => {
                idx += 1;
                options.base = Some(value(&args, idx, "--base")?.to_string());
            }
            arg if arg.starts_with("--base=") => {
                options.base = Some(inline_value(arg, "--base")?.to_string());
            }
            "--diff" => {
                idx += 1;
                options.diff = Some(parse_diff_input(value(&args, idx, "--diff")?));
            }
            arg if arg.starts_with("--diff=") => {
                options.diff = Some(parse_diff_input(inline_value(arg, "--diff")?));
            }
            "--author" => {
                idx += 1;
                author = Some(value(&args, idx, "--author")?.to_string());
            }
            arg if arg.starts_with("--author=") => {
                author = Some(inline_value(arg, "--author")?.to_string());
            }
            "--expires-at" => {
                idx += 1;
                options.expires_at = Some(value(&args, idx, "--expires-at")?.to_string());
            }
            arg if arg.starts_with("--expires-at=") => {
                options.expires_at = Some(inline_value(arg, "--expires-at")?.to_string());
            }
            "--timeout-seconds" => {
                idx += 1;
                options.timeout_seconds =
                    parse_timeout_seconds(value(&args, idx, "--timeout-seconds")?)?;
            }
            arg if arg.starts_with("--timeout-seconds=") => {
                options.timeout_seconds =
                    parse_timeout_seconds(inline_value(arg, "--timeout-seconds")?)?;
            }
            "--command" => {
                idx += 1;
                options.command = Some(value(&args, idx, "--command")?.to_string());
            }
            arg if arg.starts_with("--command=") => {
                options.command = Some(inline_value(arg, "--command")?.to_string());
            }
            "--out" => {
                idx += 1;
                options.out = Some(parse_path_value(&args, idx, "--out")?);
            }
            arg if arg.starts_with("--out=") => {
                options.out = Some(parse_inline_path_value(arg, "--out")?);
            }
            value if value.starts_with('-') => {
                return Err(format!("unknown confirm argument `{value}`"));
            }
            value => set_card_id(&mut card_id, value)?,
        }
        idx += 1;
    }
    options.card_id = card_id.ok_or_else(|| "missing card id".to_string())?;
    if options.base.is_some() && options.diff.is_some() {
        return Err("choose only one of --base or --diff".to_string());
    }
    if options.dry_run && allow_heavy {
        return Err(CONFIRM_MODE_CONFLICT.to_string());
    }
    if options.dry_run {
        options.author = author.unwrap_or_default();
        return Ok(options);
    }
    if !allow_heavy {
        return Err(CONFIRM_ALLOW_HEAVY_REFUSAL.to_string());
    }
    let Some(author) = author else {
        return Err(CONFIRM_AUTHOR_REQUIRED.to_string());
    };
    if author.trim().is_empty() {
        return Err(CONFIRM_AUTHOR_REQUIRED.to_string());
    }
    options.author = author;
    Ok(options)
}
