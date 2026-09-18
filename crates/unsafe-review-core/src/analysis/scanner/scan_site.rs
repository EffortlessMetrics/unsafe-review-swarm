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
    if !site_in_scope(
        input.diff,
        input.repo_mode,
        input.rel,
        line_no,
        line_no,
        &input.kind,
    ) && !input.repo_mode
    {
        return None;
    }
    let changed = site_on_added_lines(
        input.diff,
        input.repo_mode,
        input.rel,
        line_no,
        line_no,
        &input.kind,
    );

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
    if !site_in_scope(
        diff,
        repo_mode,
        rel,
        detected.line,
        detected.end_line,
        &detected.kind,
    ) && !repo_mode
    {
        return None;
    }
    let changed = site_on_added_lines(
        diff,
        repo_mode,
        rel,
        detected.line,
        detected.end_line,
        &detected.kind,
    );

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

/// Scan-inclusion gate. The six-line proximity window is preserved so nearby
/// context findings stay in the inventory; only the `changed` flag narrows.
fn site_in_scope(
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

/// The `changed` flag: exact added-line membership on diff-scoped runs.
/// `new_gaps` and the `changed_line` projection derive from this flag, so
/// only sites the diff added count as introduced. Repo-mode and diff-less
/// runs keep the historical always-true behavior.
fn site_on_added_lines(
    diff: Option<&DiffIndex>,
    repo_mode: bool,
    rel: &PathBuf,
    line: usize,
    end_line: usize,
    kind: &UnsafeSiteKind,
) -> bool {
    match diff {
        None => true,
        Some(_) if repo_mode => true,
        Some(d) => {
            if syntax_site_uses_exact_range(kind) {
                d.contains_in_range(rel, line, end_line)
            } else {
                d.contains_added_line(rel, line)
            }
        }
    }
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
