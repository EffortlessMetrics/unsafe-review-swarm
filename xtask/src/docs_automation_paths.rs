use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) fn collect_paths(path_glob: &str, ledger: &str) -> Result<Vec<PathBuf>, String> {
    let pattern_path = Path::new(path_glob);
    let directory = pattern_path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| format!("{ledger} path_glob `{path_glob}` needs a directory"))?;
    let file_pattern = pattern_path
        .file_name()
        .and_then(OsStr::to_str)
        .ok_or_else(|| format!("{ledger} path_glob `{path_glob}` needs a file pattern"))?;

    let mut paths = Vec::new();
    for entry in fs::read_dir(directory)
        .map_err(|err| format!("failed to read {}: {err}", directory.display()))?
    {
        let entry = entry.map_err(|err| {
            format!(
                "failed to read directory entry under {}: {err}",
                directory.display()
            )
        })?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if wildcard_match(file_pattern, name) && entry.path().is_file() {
            paths.push(entry.path());
        }
    }

    paths.sort();
    Ok(paths)
}

pub(crate) fn wildcard_match(pattern: &str, value: &str) -> bool {
    if !pattern.contains('*') {
        return pattern == value;
    }

    let parts = pattern.split('*').collect::<Vec<_>>();
    let mut remainder = value;
    if let Some(first) = parts.first().filter(|part| !part.is_empty()) {
        let Some(stripped) = remainder.strip_prefix(first) else {
            return false;
        };
        remainder = stripped;
    }

    let middle_end = parts.len().saturating_sub(1);
    for part in parts.iter().skip(1).take(middle_end.saturating_sub(1)) {
        if part.is_empty() {
            continue;
        }
        let Some(index) = remainder.find(part) else {
            return false;
        };
        remainder = &remainder[index + part.len()..];
    }

    if let Some(last) = parts.last().filter(|part| !part.is_empty()) {
        remainder.ends_with(last)
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::wildcard_match;

    #[test]
    fn wildcard_matching_supports_middle_and_suffix() {
        assert!(wildcard_match("foo*bar*.md", "foobazbarx.md"));
        assert!(!wildcard_match("foo*bar*.md", "barfoo.md"));
    }
}
