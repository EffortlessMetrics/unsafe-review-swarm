use crate::command::SavedOutputReceiptOptions;
use std::path::PathBuf;

pub(super) fn parse_saved_output_receipt_tool(
    args: &[String],
    idx: &mut usize,
    arg: &str,
    options: &mut SavedOutputReceiptOptions,
    allow_tool: bool,
) -> Result<bool, String> {
    match arg {
        "--tool" if allow_tool => {
            *idx += 1;
            options.tool = Some(super::value(args, *idx, "--tool")?.to_string());
            Ok(true)
        }
        _ if allow_tool && arg.starts_with("--tool=") => {
            options.tool = Some(super::inline_value(arg, "--tool")?.to_string());
            Ok(true)
        }
        _ => Ok(false),
    }
}

pub(super) fn parse_saved_output_receipt_common(
    args: &[String],
    idx: &mut usize,
    arg: &str,
    options: &mut SavedOutputReceiptOptions,
) -> Result<bool, String> {
    match arg {
        "--log" => {
            *idx += 1;
            options.log = PathBuf::from(super::value(args, *idx, "--log")?);
            Ok(true)
        }
        _ if arg.starts_with("--log=") => {
            options.log = PathBuf::from(super::inline_value(arg, "--log")?);
            Ok(true)
        }
        "--author" => {
            *idx += 1;
            options.author = super::value(args, *idx, "--author")?.to_string();
            Ok(true)
        }
        _ if arg.starts_with("--author=") => {
            options.author = super::inline_value(arg, "--author")?.to_string();
            Ok(true)
        }
        "--recorded-at" => {
            *idx += 1;
            options.recorded_at = super::value(args, *idx, "--recorded-at")?.to_string();
            Ok(true)
        }
        _ if arg.starts_with("--recorded-at=") => {
            options.recorded_at = super::inline_value(arg, "--recorded-at")?.to_string();
            Ok(true)
        }
        "--expires-at" => {
            *idx += 1;
            options.expires_at = super::value(args, *idx, "--expires-at")?.to_string();
            Ok(true)
        }
        _ if arg.starts_with("--expires-at=") => {
            options.expires_at = super::inline_value(arg, "--expires-at")?.to_string();
            Ok(true)
        }
        "--command" => {
            *idx += 1;
            options.command = super::value(args, *idx, "--command")?.to_string();
            Ok(true)
        }
        _ if arg.starts_with("--command=") => {
            options.command = super::inline_value(arg, "--command")?.to_string();
            Ok(true)
        }
        "--limitation" => {
            *idx += 1;
            options
                .limitations
                .push(super::value(args, *idx, "--limitation")?.to_string());
            Ok(true)
        }
        _ if arg.starts_with("--limitation=") => {
            options
                .limitations
                .push(super::inline_value(arg, "--limitation")?.to_string());
            Ok(true)
        }
        "--out" => {
            *idx += 1;
            options.out = Some(PathBuf::from(super::value(args, *idx, "--out")?));
            Ok(true)
        }
        _ if arg.starts_with("--out=") => {
            options.out = Some(PathBuf::from(super::inline_value(arg, "--out")?));
            Ok(true)
        }
        _ => Ok(false),
    }
}
