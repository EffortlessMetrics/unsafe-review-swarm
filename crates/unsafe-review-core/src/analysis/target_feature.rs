use super::scanner::text_detection::{LineCommentState, line_for_text_detection};
use std::collections::BTreeMap;

pub(super) type TargetFeatureCallees = BTreeMap<String, String>;

/// Per file, collect callees declared with a single-feature
/// `#[target_feature(enable = "<feature>")]` attribute, mapping callee
/// name to feature string. Multi-feature enable lists are out of scope.
pub(super) fn target_feature_callees(lines: &[&str]) -> TargetFeatureCallees {
    let mut callees = BTreeMap::new();
    // A bare name declared with two different features (for example in
    // sibling modules) is ambiguous and is never attributed.
    let mut ambiguous: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // Callees declared inside `impl`/`trait` blocks are methods, not
    // bare-callable free functions; they are not registered.
    let mut block_depth = 0isize;
    let mut method_block_depth: Option<isize> = None;
    // An impl/trait header whose opening brace lands on a later line is
    // still a method block once the brace arrives.
    let mut pending_method_header = false;
    // Brace depth is counted on comment/string-elided code so a brace
    // inside a literal or comment cannot open or close a block.
    let mut detection_state = LineCommentState::default();
    for (idx, raw) in lines.iter().enumerate() {
        let code_line = line_for_text_detection(raw, &mut detection_state);
        let trimmed_start = raw.trim_start();
        // Header prefixes (`pub`, `unsafe`) do not change block kind.
        let header = trimmed_start
            .strip_prefix("pub ")
            .unwrap_or(trimmed_start)
            .trim_start();
        let header = header
            .strip_prefix("unsafe ")
            .unwrap_or(header)
            .trim_start();
        let header = header.strip_prefix("pub ").unwrap_or(header).trim_start();
        if method_block_depth.is_none()
            && (header.starts_with("impl ")
                || header.starts_with("impl<")
                || header.starts_with("trait ")
                || header.starts_with("trait<"))
        {
            pending_method_header = true;
        }
        if pending_method_header && code_line.contains('{') {
            method_block_depth = Some(block_depth);
            pending_method_header = false;
        }
        let in_method_block = method_block_depth.is_some();
        let delta =
            code_line.matches('{').count() as isize - code_line.matches('}').count() as isize;
        if !in_method_block && let Some(feature) = parse_single_target_feature(raw) {
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
                    if ambiguous.contains(&name) {
                        // Already poisoned; stay unattributed.
                    } else if let Some(existing) = callees.get(&name) {
                        if existing != &feature {
                            callees.remove(&name);
                            ambiguous.insert(name);
                        }
                    } else {
                        callees.insert(name, feature);
                    }
                }
                break;
            }
        }
        block_depth += delta;
        if method_block_depth.is_some_and(|start| block_depth <= start) {
            method_block_depth = None;
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
    // Attribute only when every registry callee called in the expression
    // agrees on one feature; mixed-feature expressions stay unattributed.
    let mut matched: Option<(&String, &String)> = None;
    for (name, feature) in callees {
        if contains_call_name(expression, name) {
            match matched {
                None => matched = Some((name, feature)),
                Some((_, matched_feature)) if matched_feature != feature => return None,
                _ => {}
            }
        }
    }
    matched.map(|(name, feature)| (name.clone(), feature.clone()))
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
        // Bare calls only: a preceding `.` (method) or `:` (path) means
        // the name does not denote the registered free function.
        let starts_on_boundary =
            before.is_none_or(|ch| !is_ident_continue(ch) && ch != '.' && ch != ':');
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
    use super::{callee_target_feature, target_feature_callees};

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
    fn impl_method_is_not_collected_and_calls_must_be_bare() -> Result<(), String> {
        let callees = target_feature_callees(&[
            "struct W;",
            "impl W {",
            "#[target_feature(enable = \"avx2\")]",
            "fn sum_avx2(&self, data: &[f32]) -> f32 { 0.0 }",
            "}",
        ]);
        if callees.contains_key("sum_avx2") {
            return Err("impl method must not be registered as a bare callee".to_string());
        }
        let free = target_feature_callees(&[
            "#[target_feature(enable = \"avx2\")]",
            "pub fn sum_avx2(data: &[f32]) -> f32 { 0.0 }",
        ]);
        if callee_target_feature("x.sum_avx2(data)", &free).is_some() {
            return Err("method call must not be attributed to the free callee".to_string());
        }
        if callee_target_feature("other::sum_avx2(data)", &free).is_some() {
            return Err("path call must not be attributed to the free callee".to_string());
        }
        if callee_target_feature("sum_avx2(data)", &free).is_none() {
            return Err("bare call must be attributed".to_string());
        }
        Ok(())
    }

    #[test]
    fn impl_method_with_brace_on_next_line_is_not_collected() -> Result<(), String> {
        let callees = target_feature_callees(&[
            "struct W;",
            "impl W",
            "{",
            "#[target_feature(enable = \"avx2\")]",
            "fn sum_avx2(&self, data: &[f32]) -> f32 { 0.0 }",
            "}",
        ]);
        if callees.contains_key("sum_avx2") {
            return Err(
                "impl method with brace on the next line must not be registered".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn same_name_with_different_features_is_not_collected() -> Result<(), String> {
        let callees = target_feature_callees(&[
            "mod a {",
            "#[target_feature(enable = \"avx2\")]",
            "pub fn accumulate(data: &[f32]) -> f32 { 0.0 }",
            "}",
            "mod b {",
            "#[target_feature(enable = \"sse4.1\")]",
            "pub fn accumulate(data: &[f32]) -> f32 { 0.0 }",
            "}",
        ]);
        if callees.contains_key("accumulate") {
            return Err(
                "same-named callees with different features must stay unattributed".to_string(),
            );
        }
        Ok(())
    }

    #[test]
    fn unsafe_impl_method_is_not_collected() -> Result<(), String> {
        let callees = target_feature_callees(&[
            "struct W;",
            "unsafe impl Send for W {",
            "#[target_feature(enable = \"avx2\")]",
            "fn sum_avx2(&self, data: &[f32]) -> f32 { 0.0 }",
            "}",
        ]);
        if callees.contains_key("sum_avx2") {
            return Err("method inside an unsafe impl must not be registered".to_string());
        }
        Ok(())
    }

    #[test]
    fn brace_in_literal_inside_impl_does_not_end_method_block() -> Result<(), String> {
        let callees = target_feature_callees(&[
            "struct W;",
            "impl W {",
            "const END: &str = \"}\";",
            "#[target_feature(enable = \"avx2\")]",
            "fn sum_avx2(&self, data: &[f32]) -> f32 { 0.0 }",
            "}",
        ]);
        if callees.contains_key("sum_avx2") {
            return Err("literal brace must not end the impl method block early".to_string());
        }
        Ok(())
    }

    #[test]
    fn mixed_feature_expression_is_not_attributed() -> Result<(), String> {
        let callees = target_feature_callees(&[
            "#[target_feature(enable = \"avx2\")]",
            "pub fn a_avx2(data: &[f32]) -> f32 { 0.0 }",
            "#[target_feature(enable = \"sse4.1\")]",
            "pub fn b_sse(data: &[f32]) -> f32 { 0.0 }",
        ]);
        if callee_target_feature("a_avx2(x); b_sse(y)", &callees).is_some() {
            return Err("mixed-feature expression must stay unattributed".to_string());
        }
        Ok(())
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
