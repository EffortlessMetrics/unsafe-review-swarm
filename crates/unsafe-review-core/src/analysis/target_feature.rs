use std::collections::BTreeMap;

pub(super) type TargetFeatureCallees = BTreeMap<String, String>;

/// Per file, collect callees declared with a single-feature
/// `#[target_feature(enable = "<feature>")]` attribute, mapping callee
/// name to feature string. Multi-feature enable lists are out of scope.
pub(super) fn target_feature_callees(lines: &[&str]) -> TargetFeatureCallees {
    let mut callees = BTreeMap::new();
    for (idx, raw) in lines.iter().enumerate() {
        let Some(feature) = parse_single_target_feature(raw) else {
            continue;
        };
        for following in lines.iter().skip(idx + 1).take(8) {
            let trimmed = following.trim_start();
            if trimmed.is_empty()
                || trimmed.starts_with("#[")
                || trimmed.starts_with("///")
                || trimmed.starts_with("//")
            {
                continue;
            }
            if let Some(name) = parse_fn_name_simple(trimmed) {
                callees.insert(name, feature);
            }
            break;
        }
    }
    callees
}

fn parse_fn_name_simple(line: &str) -> Option<String> {
    let pos = line.find("fn ")?;
    let rest = &line[pos + 3..];
    let mut name = String::new();
    for ch in rest.chars() {
        if ch == '_' || ch.is_ascii_alphanumeric() {
            name.push(ch);
        } else {
            break;
        }
    }
    (!name.is_empty()).then_some(name)
}

/// Feature for the callee of an `UnsafeFnCall` expression, when the
/// expression calls a callee present in the registry.
pub(super) fn callee_target_feature(
    expression: &str,
    callees: &TargetFeatureCallees,
) -> Option<(String, String)> {
    for (name, feature) in callees {
        if contains_call_name(expression, name) {
            return Some((name.clone(), feature.clone()));
        }
    }
    None
}

fn parse_single_target_feature(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("#[target_feature") {
        return None;
    }
    // Only the single enable spelling counts; cfg_attr wrappers and
    // multi-feature lists stay out of the first cut.
    if trimmed.matches("enable").count() != 1 {
        return None;
    }
    let enable_pos = trimmed.find("enable")?;
    let after = &trimmed[enable_pos + "enable".len()..];
    let after = after.trim_start().strip_prefix('=')?.trim_start();
    let after = after.strip_prefix('"')?;
    let end = after.find('"')?;
    let feature = &after[..end];
    if feature.is_empty() || feature.contains(',') {
        return None;
    }
    Some(feature.to_string())
}

pub(super) fn is_target_feature_attribute(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("#[target_feature") || contains_call_name(trimmed, "target_feature")
}

fn contains_call_name(line: &str, name: &str) -> bool {
    let mut cursor = line;
    while let Some(pos) = cursor.find(name) {
        let before = cursor[..pos].chars().next_back();
        let after = &cursor[pos + name.len()..];
        let starts_on_boundary = before.is_none_or(|ch| !is_ident_continue(ch));
        if starts_on_boundary && call_suffix(after) {
            return true;
        }
        cursor = &after[after
            .char_indices()
            .next()
            .map_or(after.len(), |(idx, ch)| idx + ch.len_utf8())..];
    }
    false
}

fn call_suffix(after_name: &str) -> bool {
    let rest = after_name.trim_start();
    if rest.starts_with('(') {
        return true;
    }
    rest.strip_prefix("::")
        .is_some_and(|after_colons| after_colons.trim_start().starts_with('<'))
}

fn is_ident_continue(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::target_feature_callees;

    #[test]
    fn collects_single_feature_callee() -> Result<(), String> {
        let callees = target_feature_callees(&[
            "#[target_feature(enable = \"avx2\")]",
            "pub fn sum_avx2(data: &[f32]) -> f32 {",
        ]);
        match callees.get("sum_avx2").map(String::as_str) {
            Some("avx2") => Ok(()),
            other => Err(format!("expected avx2 for sum_avx2, got {other:?}")),
        }
    }

    #[test]
    fn multi_feature_callee_is_not_collected() -> Result<(), String> {
        let callees = target_feature_callees(&[
            "#[target_feature(enable = \"avx2\", enable = \"sse4.1\")]",
            "pub fn sum_multi(data: &[f32]) -> f32 {",
        ]);
        if callees.contains_key("sum_multi") {
            return Err("multi-feature callee must not be collected".to_string());
        }
        Ok(())
    }
}
