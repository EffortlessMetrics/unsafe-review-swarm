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

fn parse_max_cards_arg(args: &[String], idx: usize, flag: &str) -> Result<usize, String> {
    parse_max_cards(value(args, idx, flag)?)
}

fn parse_max_cards_inline(arg: &str, flag: &str) -> Result<usize, String> {
    parse_max_cards(inline_value(arg, flag)?)
}
