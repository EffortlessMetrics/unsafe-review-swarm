use std::path::{Path, PathBuf};

pub(crate) fn link_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut rest = text;
    while let Some(label_start) = rest.find('[') {
        rest = &rest[label_start + 1..];
        let Some(label_end) = rest.find(']') else {
            break;
        };
        let after_label = &rest[label_end + 1..];
        let Some(after_open) = after_label.strip_prefix('(') else {
            rest = after_label;
            continue;
        };
        let Some(target_end) = after_open.find(')') else {
            break;
        };
        let target = after_open[..target_end].trim();
        if !target.is_empty() {
            targets.push(target.to_string());
        }
        rest = &after_open[target_end + 1..];
    }
    targets
}

pub(crate) fn local_link_target(target: &str) -> Option<&str> {
    let target = target
        .split_once('#')
        .map_or(target, |(path, _)| path)
        .trim();
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with("file:")
        || target.starts_with("sandbox:")
    {
        return None;
    }
    Some(target)
}

pub(crate) fn link_path(source: &Path, target: &str) -> PathBuf {
    let target_path = Path::new(target);
    if target_path.is_absolute() {
        return target_path.to_path_buf();
    }
    source.parent().map_or_else(
        || target_path.to_path_buf(),
        |parent| parent.join(target_path),
    )
}

pub(crate) fn code_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut in_code = false;
    for ch in text.chars() {
        if ch == '`' {
            if in_code {
                spans.push(current.clone());
                current.clear();
            }
            in_code = !in_code;
        } else if in_code {
            current.push(ch);
        }
    }
    spans
}
