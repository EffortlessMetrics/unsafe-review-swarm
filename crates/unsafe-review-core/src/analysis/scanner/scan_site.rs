use super::*;

/// Recovers the name a statement binds an unsafe-block value to, e.g.
/// `result` in `let result = unsafe { checkable() };`. Returns `None` for
/// discarded (`let _ =`), ignored (bare statement), or nested values, and for
/// non-plain bindings. Only the binding prefix on the site line (or a trailing
/// `let NAME =` on the previous line) counts; anything else is silence, not a
/// missing discharge.
pub(super) fn call_bound_name(lines: &[&str], idx: usize) -> Option<String> {
    let line = lines.get(idx)?;
    if let Some(name) = bound_name_on_line(line) {
        return Some(name);
    }
    if idx == 0 {
        return None;
    }
    // Multiline binding: `let NAME [: TYPE] =` with the unsafe value on the
    // next (site) line.
    let prev = lines[idx - 1].trim();
    let prefix = prev.strip_suffix('=')?;
    if prefix.ends_with(['=', '!', '<', '>']) {
        return None;
    }
    let_prefix_name(prefix)
}

fn bound_name_on_line(line: &str) -> Option<String> {
    let code = line.split("//").next().unwrap_or(line);
    let (prefix, suffix) = split_plain_assignment(code)?;
    if !suffix.contains("unsafe") {
        return None;
    }
    if prefix.contains(['(', ')', ';', '{', '}', '"']) {
        return None;
    }
    if let Some(name) = let_prefix_name(prefix) {
        return Some(name);
    }
    // A plain `NAME =` reassignment also captures the value.
    let name = prefix.trim();
    if is_plain_identifier(name) && name != "_" {
        return Some(name.to_string());
    }
    None
}

/// Extracts the bound name from a `let [mut ]NAME [: TYPE]` prefix.
/// Returns `None` for discards, missing names, and non-identifier targets.
fn let_prefix_name(prefix: &str) -> Option<String> {
    let rest = prefix.trim().strip_prefix("let ").unwrap_or(prefix.trim());
    if rest == prefix.trim() {
        // No `let` keyword; not a let prefix.
        return None;
    }
    let rest = rest
        .strip_prefix("mut ")
        .map(str::trim)
        .unwrap_or_else(|| rest.trim());
    let name = rest.split(':').next()?.trim();
    if name == "_" || !is_plain_identifier(name) {
        return None;
    }
    Some(name.to_string())
}

/// Splits `code` at the first plain `=` assignment operator. Returns `None`
/// for comparisons, arrows, fat arrows, and missing operators.
fn split_plain_assignment(code: &str) -> Option<(&str, &str)> {
    let bytes = code.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'=' {
            let prev = if i > 0 { bytes[i - 1] } else { b' ' };
            let next = bytes.get(i + 1).copied().unwrap_or(b' ');
            if matches!(prev, b'=' | b'!' | b'<' | b'>') || matches!(next, b'=' | b'>') {
                i += 1;
                continue;
            }
            return Some((&code[..i], &code[i + 1..]));
        }
        i += 1;
    }
    None
}

fn is_plain_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(first) if first.is_ascii_alphabetic() || first == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// After-context window for a site. FFI call sites get an extended window
/// (up to 24 lines) so post-call return-value checks stay visible; the window
/// still ends at the enclosing top-level item (first column-zero `}`), so one
/// item's checks cannot leak into the next. All other sites keep the uniform
/// 7-line window.
pub(super) fn context_after_for(lines: &[&str], idx: usize, extended: bool) -> Vec<String> {
    let start = idx + 1;
    if start >= lines.len() {
        return Vec::new();
    }
    let mut end = (idx + 8).min(lines.len());
    if extended {
        end = (idx + 25).min(lines.len());
        for (offset, line) in lines.iter().enumerate().skip(start).take(24) {
            if *line == "}" {
                end = end.min(offset);
                break;
            }
        }
    }
    context_slice(lines, start, end)
}

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
    let is_ffi_call = input.kind == UnsafeSiteKind::FfiCall;
    let bound_name = if is_ffi_call {
        call_bound_name(input.lines, input.idx)
    } else {
        None
    };
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
            bound_name,
        },
        context_before: context_before_site(input.lines, input.idx),
        context_after: context_after_for(input.lines, input.idx, is_ffi_call),
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
    let is_ffi_call = detected.kind == UnsafeSiteKind::FfiCall;
    let bound_name = if is_ffi_call {
        call_bound_name(lines, idx)
    } else {
        None
    };
    let context_after = context_after_for(lines, idx, is_ffi_call);

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
            bound_name,
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
