#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

pub(crate) fn require_no_extra_args(args: &[String], command: &str) -> Result<(), String> {
    require_max_args(args, command, 2)
}

pub(crate) fn require_max_args(
    args: &[String],
    command: &str,
    max_len: usize,
) -> Result<(), String> {
    if args.len() <= max_len {
        return Ok(());
    }
    Err(format!(
        "`{command}` does not accept extra arguments: {}",
        args[max_len..].join(" ")
    ))
}

pub(crate) fn require_subcommand_dir_arg(
    args: &[String],
    command: &str,
) -> Result<PathBuf, String> {
    let Some(dir) = args.get(2) else {
        return Err(format!("usage: cargo xtask {command} <dir>"));
    };
    require_max_args(args, command, 3)?;
    Ok(Path::new(dir).to_path_buf())
}
