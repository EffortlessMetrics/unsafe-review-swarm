use crate::analysis::scanner;
use crate::domain::{CardId, HazardKind, OperationFamily, UnsafeSiteKind};
use crate::util::{slug, stable_hash_hex};
use std::collections::BTreeMap;

pub(super) fn card_id(
    package: &str,
    scanned: &scanner::ScannedSite,
    hazards: &[HazardKind],
    identity_counts: &mut BTreeMap<String, usize>,
) -> CardId {
    let base = card_identity_base(package, scanned, hazards);
    let next = identity_counts.entry(base.clone()).or_insert(0);
    *next += 1;
    CardId(format!("{base}-c{}", *next))
}

fn card_identity_base(
    package: &str,
    scanned: &scanner::ScannedSite,
    hazards: &[HazardKind],
) -> String {
    let file = scanned
        .site
        .location
        .file
        .to_string_lossy()
        .replace(['/', '\\'], "_");
    let owner = scanned
        .site
        .owner
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let normalized = normalize_snippet(&identity_expression(scanned));
    let snippet_hash = stable_hash_hex(&normalized);
    let hazard = hazards.first().map_or("unknown", |hazard| hazard.as_str());
    format!(
        "UR-{}-{}-{}-{}-{}-{}-{}-{}",
        slug(package),
        slug(&file),
        slug(&owner),
        scanned.site.kind.as_str(),
        scanned.operation.family.as_str(),
        slug(&operation_path(scanned)),
        &snippet_hash[..12],
        hazard
    )
}

fn normalize_snippet(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn identity_expression(scanned: &scanner::ScannedSite) -> String {
    if scanned.site.kind == UnsafeSiteKind::ExternBlock
        && scanned.operation.family == OperationFamily::Ffi
    {
        let mut lines = vec![scanned.operation.expression.clone()];
        for line in &scanned.context_after {
            lines.push(line.clone());
            if line.contains('}') {
                break;
            }
        }
        return lines.join(" ");
    }
    scanned.operation.expression.clone()
}

fn operation_path(scanned: &scanner::ScannedSite) -> String {
    if scanned.operation.family == OperationFamily::RawPointerDeref {
        return "deref".to_string();
    }
    if scanned.operation.family == OperationFamily::UnreachableUnchecked {
        return "unreachable_unchecked".to_string();
    }
    if scanned.operation.family == OperationFamily::UnsafeFnCall {
        return unsafe_call_path(&scanned.operation.expression);
    }
    if scanned.operation.family == OperationFamily::Unknown {
        return scanned
            .site
            .owner
            .clone()
            .unwrap_or_else(|| scanned.site.kind.as_str().to_string());
    }
    let normalized = normalize_snippet(&scanned.operation.expression);
    let target = normalized
        .split('(')
        .next()
        .unwrap_or(normalized.as_str())
        .trim();
    if let Some((_prefix, method)) = target.rsplit_once('.') {
        return method.trim_matches(':').to_string();
    }
    if let Some((_prefix, function)) = target.rsplit_once("::") {
        return function.trim_matches(':').to_string();
    }
    scanned.operation.family.as_str().to_string()
}

pub(super) fn unsafe_call_path(expression: &str) -> String {
    let normalized = normalize_snippet(expression);
    if contains_call_name(&normalized, "new_unchecked") {
        return "new_unchecked".to_string();
    }
    let call = normalized
        .split_once("unsafe")
        .and_then(|(_prefix, after_unsafe)| {
            after_unsafe.split_once('{').map(|(_open, after)| after)
        })
        .unwrap_or(normalized.as_str())
        .split('(')
        .next()
        .unwrap_or("unsafe_fn_call")
        .trim()
        .trim_start_matches("match")
        .trim();
    let call = strip_trailing_turbofish(call);
    if call.is_empty() {
        "unsafe_fn_call".to_string()
    } else if let Some((_prefix, method)) = call.rsplit_once('.') {
        method.trim_matches(':').to_string()
    } else if let Some((_prefix, function)) = call.rsplit_once("::") {
        function.trim_matches(':').to_string()
    } else {
        call.trim_matches(':').to_string()
    }
}

fn strip_trailing_turbofish(call: &str) -> &str {
    let call = call.trim();
    if !call.ends_with('>') {
        return call;
    }

    let mut depth = 0usize;
    for (idx, ch) in call.char_indices().rev() {
        match ch {
            '>' => depth += 1,
            '<' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let prefix = &call[..idx];
                    if let Some(without_colons) = prefix.strip_suffix("::") {
                        return without_colons;
                    }
                    return call;
                }
            }
            _ => {}
        }
    }

    call
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
