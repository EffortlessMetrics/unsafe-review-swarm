use super::*;

pub(super) struct FallbackSiteInput<'a> {
    pub(super) rel: &'a PathBuf,
    pub(super) diff: Option<&'a DiffIndex>,
    pub(super) repo_mode: bool,
    pub(super) lines: &'a [&'a str],
    pub(super) idx: usize,
    pub(super) raw: &'a str,
    pub(super) trimmed: &'a str,
    pub(super) detection_trimmed: &'a str,
    pub(super) kind: UnsafeSiteKind,
    pub(super) family: OperationFamily,
}

pub(super) fn fallback_site(input: FallbackSiteInput<'_>) -> Option<ScannedSite> {
    let line_no = input.idx + 1;
    let changed = site_changed(
        input.diff,
        input.repo_mode,
        input.rel,
        line_no,
        line_no,
        &input.kind,
    );
    if !changed && !input.repo_mode {
        return None;
    }

    let owner = fallback_owner(
        input.lines,
        input.idx,
        input.detection_trimmed,
        &input.kind,
        &input.family,
    );
    let public_api_surface = is_public_api_surface(&input.kind, input.trimmed);
    Some(ScannedSite {
        site: UnsafeSite {
            location: SourceLocation::new(
                input.rel.clone(),
                line_no,
                first_non_ws_column(input.raw),
            ),
            kind: input.kind,
            owner,
            visibility: visibility_for_snippet(input.trimmed).to_string(),
            public_api_surface,
            changed,
            snippet: input.trimmed.to_string(),
        },
        operation: UnsafeOperation {
            family: input.family,
            expression: input.trimmed.to_string(),
        },
        context_before: context_before_site(input.lines, input.idx),
        context_after: context_slice(
            input.lines,
            input.idx + 1,
            (input.idx + 8).min(input.lines.len()),
        ),
    })
}

pub(super) fn syntax_site(
    rel: &PathBuf,
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    lines: &[&str],
    detected: DetectedSyntaxSite,
) -> Option<ScannedSite> {
    let changed = site_changed(
        diff,
        repo_mode,
        rel,
        detected.line,
        detected.end_line,
        &detected.kind,
    );
    if !changed && !repo_mode {
        return None;
    }

    let idx = detected.line.saturating_sub(1);
    let owner = syntax_owner(&detected, lines, idx);
    let visibility = visibility_for_snippet(&detected.source_snippet).to_string();
    let public_api_surface = is_public_api_surface(&detected.kind, &detected.source_snippet);
    let context_before = context_before_site(lines, idx);
    let context_after = context_slice(
        lines,
        (idx + 1).min(lines.len()),
        (idx + 8).min(lines.len()),
    );

    Some(ScannedSite {
        site: UnsafeSite {
            location: SourceLocation::new(rel.clone(), detected.line, detected.column),
            kind: detected.kind,
            owner,
            visibility,
            public_api_surface,
            changed,
            snippet: detected.card_snippet.clone(),
        },
        operation: UnsafeOperation {
            family: detected.family,
            expression: detected.card_snippet,
        },
        context_before,
        context_after,
    })
}

fn site_changed(
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    rel: &PathBuf,
    line: usize,
    end_line: usize,
    kind: &UnsafeSiteKind,
) -> bool {
    diff.is_none_or(|d| {
        repo_mode
            || if syntax_site_uses_exact_range(kind) {
                d.contains_in_range(rel, line, end_line)
            } else {
                d.contains_near(rel, line)
            }
    })
}

fn fallback_owner(
    lines: &[&str],
    idx: usize,
    detection_trimmed: &str,
    kind: &UnsafeSiteKind,
    family: &OperationFamily,
) -> Option<String> {
    match (kind, family) {
        (UnsafeSiteKind::ExternBlock, OperationFamily::Ffi) => find_extern_block_owner(lines, idx),
        (UnsafeSiteKind::Operation, OperationFamily::TargetFeature) => {
            find_following_fn_owner(lines, idx)
        }
        (UnsafeSiteKind::StaticMut, OperationFamily::StaticMut) => {
            parse_static_mut_name(detection_trimmed)
        }
        _ => None,
    }
    .or_else(|| find_owner(lines, idx))
}
