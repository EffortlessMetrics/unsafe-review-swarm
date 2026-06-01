use super::*;

pub(super) fn next_string(args: &[String], idx: &mut usize, flag: &str) -> Result<String, String> {
    *idx += 1;
    Ok(value(args, *idx, flag)?.to_string())
}

pub(super) fn inline_string(arg: &str, flag: &str) -> Result<String, String> {
    Ok(inline_value(arg, flag)?.to_string())
}

pub(super) fn next_path(args: &[String], idx: &mut usize, flag: &str) -> Result<PathBuf, String> {
    next_string(args, idx, flag).map(PathBuf::from)
}

pub(super) fn inline_path(arg: &str, flag: &str) -> Result<PathBuf, String> {
    inline_value(arg, flag).map(PathBuf::from)
}
