mod card_builder;

use super::{receipts, scanner};
use crate::api::{
    AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, DiscoveryOptions, RepoScanEvent,
    RepoScanPhase, RepoScanStatus, Scope, Summary,
};
use crate::domain::{CardId, ReviewCard};
use crate::input::{diff, workspace};
use crate::policy::PolicyState;
use crate::util::{slug, stable_hash_hex};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

type RepoEventFn<'a> = &'a mut dyn FnMut(&RepoScanEvent) -> Result<(), String>;

pub(crate) fn analyze(input: AnalyzeInput) -> Result<AnalyzeOutput, String> {
    let discovery = default_discovery_for(&input);
    analyze_with_receipts(input, true, discovery, None)
}

pub(crate) fn analyze_with_discovery(
    input: AnalyzeInput,
    discovery: DiscoveryOptions,
) -> Result<AnalyzeOutput, String> {
    analyze_with_receipts(input, true, discovery, None)
}

pub(crate) fn analyze_with_discovery_and_progress<F>(
    input: AnalyzeInput,
    discovery: DiscoveryOptions,
    mut progress: F,
) -> Result<AnalyzeOutput, String>
where
    F: FnMut(&RepoScanStatus) -> Result<(), String>,
{
    let mut events = |event: &RepoScanEvent| progress(&event.status);
    analyze_with_receipts(input, true, discovery, Some(&mut events))
}

pub(crate) fn analyze_with_discovery_and_repo_events<F>(
    input: AnalyzeInput,
    discovery: DiscoveryOptions,
    mut events: F,
) -> Result<AnalyzeOutput, String>
where
    F: FnMut(&RepoScanEvent) -> Result<(), String>,
{
    analyze_with_receipts(input, true, discovery, Some(&mut events))
}

pub(crate) fn analyze_without_receipts(input: AnalyzeInput) -> Result<AnalyzeOutput, String> {
    let discovery = default_discovery_for(&input);
    analyze_with_receipts(input, false, discovery, None)
}

fn default_discovery_for(input: &AnalyzeInput) -> DiscoveryOptions {
    if matches!(input.scope, Scope::Repo) || matches!(input.mode, AnalysisMode::Repo) {
        DiscoveryOptions::repo_defaults()
    } else {
        DiscoveryOptions::default()
    }
}

fn analyze_with_receipts(
    input: AnalyzeInput,
    import_receipts: bool,
    discovery: DiscoveryOptions,
    mut events: Option<RepoEventFn<'_>>,
) -> Result<AnalyzeOutput, String> {
    let started = Instant::now();
    let repo_mode = matches!(input.scope, Scope::Repo) || matches!(input.mode, AnalysisMode::Repo);
    let diff_index = load_diff_index(&input.diff)?;
    emit_repo_status(
        &mut events,
        repo_status(RepoScanPhase::Discovering, &started, 0, 0, 0, None, false),
    )?;
    let mut discovered_files = 0usize;
    let all_rust_files = {
        let mut discovery_progress = |count: usize, path: &Path| {
            discovered_files = count;
            emit_repo_status(
                &mut events,
                repo_status(
                    RepoScanPhase::Discovering,
                    &started,
                    discovered_files,
                    0,
                    0,
                    Some(path.to_path_buf()),
                    false,
                ),
            )
        };
        workspace::discover_rust_files_with_progress(
            &input.root,
            &discovery,
            Some(&mut discovery_progress),
        )?
    };
    discovered_files = all_rust_files.len();
    let package = package_name(&input.root);
    let policy_state = PolicyState::load(&input.root)?;
    let receipt_index = if import_receipts {
        receipts::ReceiptIndex::load(&input.root)?
    } else {
        receipts::ReceiptIndex::default()
    };
    let candidate_files = if repo_mode || diff_index.is_empty() {
        all_rust_files.clone()
    } else {
        all_rust_files
            .iter()
            .filter(|path| diff_index.contains_file(path))
            .cloned()
            .collect::<Vec<_>>()
    };

    let mut cards = Vec::new();
    let mut identity_counts = BTreeMap::new();
    let max_cards = input.max_cards.unwrap_or(usize::MAX);
    let mut files_scanned = 0usize;
    let mut last_scanned_path = None;
    emit_repo_status(
        &mut events,
        repo_status(
            RepoScanPhase::Scanning,
            &started,
            discovered_files,
            files_scanned,
            cards.len(),
            None,
            false,
        ),
    )?;
    'files: for rel in &candidate_files {
        if cards.len() >= max_cards {
            break;
        }
        emit_repo_status(
            &mut events,
            repo_status(
                RepoScanPhase::Scanning,
                &started,
                discovered_files,
                files_scanned,
                cards.len(),
                Some(rel.clone()),
                false,
            ),
        )?;
        let scanned = scanner::scan_file(&input.root, rel, Some(&diff_index), repo_mode)?;
        files_scanned += 1;
        let mut build_ctx = card_builder::CardBuildContext {
            root: &input.root,
            package: &package,
            receipt_index: &receipt_index,
            policy_state: &policy_state,
            identity_counts: &mut identity_counts,
        };
        let mut reached_max_cards = false;
        for scanned_site in scanned {
            cards.push(card_builder::build_card(&mut build_ctx, scanned_site));
            if cards.len() >= max_cards {
                reached_max_cards = true;
                break;
            }
        }
        emit_repo_event(
            &mut events,
            repo_status(
                RepoScanPhase::Scanning,
                &started,
                discovered_files,
                files_scanned,
                cards.len(),
                Some(rel.clone()),
                false,
            ),
            Some(partial_analyze_output(
                &input,
                all_rust_files.len(),
                candidate_files.len(),
                &cards,
            )),
        )?;
        last_scanned_path = Some(rel.clone());
        if reached_max_cards {
            break 'files;
        }
    }
    sort_cards(&mut cards);
    let summary = summarize(all_rust_files.len(), candidate_files.len(), &cards);
    let output = AnalyzeOutput {
        schema_version: "0.1".to_string(),
        tool: "unsafe-review".to_string(),
        root: input.root.clone(),
        scope: input.scope.clone(),
        mode: input.mode.clone(),
        policy: input.policy.clone(),
        summary,
        cards,
    };
    emit_repo_event(
        &mut events,
        repo_status(
            RepoScanPhase::Complete,
            &started,
            discovered_files,
            files_scanned,
            output.cards.len(),
            last_scanned_path,
            true,
        ),
        Some(output.clone()),
    )?;
    Ok(output)
}

fn sort_cards(cards: &mut [ReviewCard]) {
    cards.sort_by(|left, right| {
        left.site
            .location
            .file
            .cmp(&right.site.location.file)
            .then(left.site.location.line.cmp(&right.site.location.line))
    });
}

fn partial_analyze_output(
    input: &AnalyzeInput,
    rust_files: usize,
    changed_rust_files: usize,
    cards: &[ReviewCard],
) -> AnalyzeOutput {
    let mut cards = cards.to_vec();
    sort_cards(&mut cards);
    let summary = summarize(rust_files, changed_rust_files, &cards);
    AnalyzeOutput {
        schema_version: "0.1".to_string(),
        tool: "unsafe-review".to_string(),
        root: input.root.clone(),
        scope: input.scope.clone(),
        mode: input.mode.clone(),
        policy: input.policy.clone(),
        summary,
        cards,
    }
}

fn emit_repo_status(
    events: &mut Option<RepoEventFn<'_>>,
    status: RepoScanStatus,
) -> Result<(), String> {
    emit_repo_event(events, status, None)
}

fn emit_repo_event(
    events: &mut Option<RepoEventFn<'_>>,
    status: RepoScanStatus,
    partial_output: Option<AnalyzeOutput>,
) -> Result<(), String> {
    if let Some(events) = events.as_deref_mut() {
        events(&RepoScanEvent {
            status,
            partial_output,
        })?;
    }
    Ok(())
}

fn repo_status(
    phase: RepoScanPhase,
    started: &Instant,
    files_discovered: usize,
    files_scanned: usize,
    cards_found: usize,
    last_path: Option<PathBuf>,
    completed: bool,
) -> RepoScanStatus {
    RepoScanStatus {
        schema_version: "repo-scan-status/v1".to_string(),
        phase,
        elapsed_ms: started.elapsed().as_millis().try_into().unwrap_or(u64::MAX),
        files_discovered,
        files_scanned,
        cards_found,
        last_path,
        completed,
    }
}

fn package_name(root: &std::path::Path) -> String {
    let Ok(text) = fs::read_to_string(root.join("Cargo.toml")) else {
        return root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("workspace")
            .to_string();
    };
    let mut in_package = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_package = trimmed == "[package]";
            continue;
        }
        if !in_package || !trimmed.starts_with("name") {
            continue;
        }
        let Some((_key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let name = value.trim().trim_matches('"');
        if !name.is_empty() {
            return name.to_string();
        }
    }
    root.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("workspace")
        .to_string()
}

fn load_diff_index(source: &DiffSource) -> Result<diff::DiffIndex, String> {
    match source {
        DiffSource::NoneRepoScan => Ok(diff::DiffIndex::default()),
        DiffSource::Text(text) => Ok(diff::parse_unified_diff(text)),
        DiffSource::File(path) => {
            let text = fs::read_to_string(path)
                .map_err(|err| format!("read diff {} failed: {err}", path.display()))?;
            Ok(diff::parse_unified_diff(&text))
        }
    }
}

fn summarize(rust_files: usize, changed_rust_files: usize, cards: &[ReviewCard]) -> Summary {
    let mut summary = Summary {
        rust_files,
        changed_rust_files,
        unsafe_sites: cards.len(),
        cards: cards.len(),
        ..Summary::default()
    };
    for card in cards {
        if card.class.is_actionable() {
            summary.open_actionable_gaps += 1;
        }
        match &card.class {
            crate::domain::ReviewClass::ContractMissing => summary.contract_missing += 1,
            crate::domain::ReviewClass::GuardMissing => summary.guard_missing += 1,
            crate::domain::ReviewClass::GuardedUnwitnessed => summary.guarded_unwitnessed += 1,
            crate::domain::ReviewClass::UnsafeUnreached => summary.unsafe_unreached += 1,
            crate::domain::ReviewClass::RequiresLoom => summary.requires_loom += 1,
            crate::domain::ReviewClass::MiriUnsupported => summary.miri_unsupported += 1,
            crate::domain::ReviewClass::StaticUnknown => summary.static_unknown += 1,
            _ => {}
        }
    }
    summary
}

fn next_action_summary(
    class: &crate::domain::ReviewClass,
    operation: &str,
    public_api_surface: bool,
    routes: &[crate::domain::WitnessRoute],
) -> String {
    match class {
        crate::domain::ReviewClass::ContractMissing if public_api_surface => {
            "Add a precise public `# Safety` section that names the required caller obligations."
                .to_string()
        }
        crate::domain::ReviewClass::ContractMissing => "Add a precise `# Safety` section or `SAFETY:` / `Safety:` comment that names the required conditions.".to_string(),
        crate::domain::ReviewClass::GuardMissing if operation == "unknown" => "Review the unsafe site manually and add the missing obligation-specific guard once the contract is identified.".to_string(),
        crate::domain::ReviewClass::GuardMissing if operation == "unsafe_fn_call" => "Review the `unsafe_fn_call` callee contract manually and add obligation-specific guard evidence for this call.".to_string(),
        crate::domain::ReviewClass::GuardMissing if operation == "inline_asm" => "Review the `inline_asm` register, memory, and target invariants manually; add explicit guard evidence, and attach a human deep-review receipt only as witness evidence.".to_string(),
        crate::domain::ReviewClass::GuardMissing if operation == "pin_unchecked" => "Review the `pin_unchecked` move-prevention and projection invariants manually; add explicit guard evidence, and attach a human deep-review receipt only as witness evidence.".to_string(),
        crate::domain::ReviewClass::GuardMissing => format!("Add or expose the local guard that discharges the `{operation}` safety obligation."),
        crate::domain::ReviewClass::GuardedUnwitnessed
            if has_witness_route(routes, crate::domain::WitnessKind::HumanDeepReview) =>
        {
            "Attach a human deep-review witness receipt or mark the static limitation explicitly."
                .to_string()
        }
        crate::domain::ReviewClass::GuardedUnwitnessed
            if has_witness_route(routes, crate::domain::WitnessKind::Miri)
                && has_witness_route(routes, crate::domain::WitnessKind::CargoCareful) =>
        {
            "Attach a focused Miri or cargo-careful witness receipt or mark the static limitation explicitly.".to_string()
        }
        crate::domain::ReviewClass::GuardedUnwitnessed
            if has_witness_route(routes, crate::domain::WitnessKind::Miri) =>
        {
            "Attach a focused Miri witness receipt or mark the static limitation explicitly."
                .to_string()
        }
        crate::domain::ReviewClass::GuardedUnwitnessed
            if has_witness_route(routes, crate::domain::WitnessKind::CargoCareful) =>
        {
            "Attach a focused cargo-careful witness receipt or mark the static limitation explicitly.".to_string()
        }
        crate::domain::ReviewClass::ReachableUnwitnessed => "Attach a focused witness receipt for the reached unsafe seam or mark the static limitation explicitly.".to_string(),
        crate::domain::ReviewClass::WitnessMismatch => "Review the witness identity or tool mismatch and attach a matching receipt for this card.".to_string(),
        crate::domain::ReviewClass::RequiresLoom => "Add or update a Loom/Shuttle model for the changed concurrency invariant.".to_string(),
        crate::domain::ReviewClass::RequiresSanitizer => "Run a focused sanitizer or cargo-careful witness and attach the receipt with limitations.".to_string(),
        crate::domain::ReviewClass::RequiresKaniOrCrux => "Run a bounded Kani/Crux proof harness or attach the receipt with limitations.".to_string(),
        crate::domain::ReviewClass::MiriUnsupported => "Use sanitizer/cargo-careful or an explicit FFI boundary contract; Miri may not exercise this seam.".to_string(),
        crate::domain::ReviewClass::StaticUnknown => "Review the unsafe site manually; identify the missing contract, guard, test, or witness route before claiming progress.".to_string(),
        crate::domain::ReviewClass::UnsafeUnreached => "Add or identify a focused test path that reaches the safe wrapper around this unsafe seam.".to_string(),
        crate::domain::ReviewClass::BaselineKnown => "Known baseline card; keep the ledger owner and review date current.".to_string(),
        crate::domain::ReviewClass::Suppressed => "Suppressed card; keep the owner, reason, evidence, and review or expiry date current.".to_string(),
        _ => "Attach a focused witness receipt or mark the static limitation explicitly.".to_string(),
    }
}

fn has_witness_route(
    routes: &[crate::domain::WitnessRoute],
    kind: crate::domain::WitnessKind,
) -> bool {
    routes.iter().any(|route| route.kind == kind)
}

fn card_id(
    package: &str,
    scanned: &scanner::ScannedSite,
    hazards: &[crate::domain::HazardKind],
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
    hazards: &[crate::domain::HazardKind],
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
    let normalized = normalize_snippet(&scanned.operation.expression);
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

fn operation_path(scanned: &scanner::ScannedSite) -> String {
    if scanned.operation.family == crate::domain::OperationFamily::RawPointerDeref {
        return "deref".to_string();
    }
    if scanned.operation.family == crate::domain::OperationFamily::UnreachableUnchecked {
        return "unreachable_unchecked".to_string();
    }
    if scanned.operation.family == crate::domain::OperationFamily::UnsafeFnCall {
        return unsafe_call_path(&scanned.operation.expression);
    }
    if scanned.operation.family == crate::domain::OperationFamily::Unknown {
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

fn unsafe_call_path(expression: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, DiffSource, DiscoveryOptions, PolicyMode, Scope};
    use crate::domain::{
        HazardKind, OperationFamily, Priority, ReviewCard, ReviewClass, UnsafeSiteKind,
        WitnessKind, WitnessRoute,
    };
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_witness_route(kind: WitnessKind) -> WitnessRoute {
        WitnessRoute {
            kind,
            reason: "test route".to_string(),
            command: None,
            required: false,
        }
    }

    #[test]
    fn raw_pointer_v1_operation_cards_are_concrete() -> Result<(), String> {
        let cases = [
            (
                "raw_pointer_alignment",
                OperationFamily::RawPointerRead,
                true,
            ),
            ("raw_pointer_deref", OperationFamily::RawPointerDeref, true),
            (
                "raw_pointer_read_unaligned",
                OperationFamily::RawPointerReadUnaligned,
                false,
            ),
            (
                "raw_pointer_read_volatile",
                OperationFamily::RawPointerRead,
                true,
            ),
            (
                "raw_pointer_write_assignment",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_unaligned",
                OperationFamily::RawPointerWriteUnaligned,
                false,
            ),
            (
                "raw_pointer_write_bytes",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_bool_bytes_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_bool_reassigned_byte_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_bool_closed_branch_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_previous_slice_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_previous_u8_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_previous_bool_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_previous_maybeuninit_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_other_u8_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_maybeuninit",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_other_maybeuninit_not_guard",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "raw_pointer_write_volatile",
                OperationFamily::RawPointerWrite,
                true,
            ),
            (
                "split_raw_pointer_read_call",
                OperationFamily::RawPointerRead,
                true,
            ),
        ];

        for (fixture, expected_family, expects_alignment) in cases {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(
                card.operation.family, expected_family,
                "{fixture} should emit the concrete operation family"
            );
            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert_eq!(
                card.hazards.contains(&HazardKind::Alignment),
                expects_alignment,
                "{fixture} alignment hazard expectation drifted"
            );
            assert!(card.contract.present);
            assert_eq!(card.reach.state, "owner_reached");
            assert!(card.missing.iter().any(|missing| missing.kind == "guard"));
            assert!(
                card.next_action
                    .verify_commands
                    .iter()
                    .any(|command| command.contains("miri test")),
                "{fixture} should recommend a concrete Miri witness"
            );
            assert_no_unknown_wrapper_card(fixture, &output);
        }
        Ok(())
    }

    #[test]
    fn unaligned_raw_pointer_read_does_not_require_alignment_guard() -> Result<(), String> {
        let output = fixture_output("raw_pointer_read_unaligned")?;
        let card = single_card("raw_pointer_read_unaligned", &output)?;

        assert_eq!(
            card.operation.family,
            OperationFamily::RawPointerReadUnaligned
        );
        assert!(!card.hazards.contains(&HazardKind::Alignment));
        assert!(
            card.obligations
                .iter()
                .all(|obligation| obligation.key != "alignment")
        );
        assert!(card.contract.present);
        assert!(
            obligation_discharge_present(card, "bounds"),
            "length evidence should still satisfy the bounds obligation"
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        Ok(())
    }

    #[test]
    fn unaligned_raw_pointer_write_does_not_require_alignment_guard() -> Result<(), String> {
        let output = fixture_output("raw_pointer_write_unaligned")?;
        let card = single_card("raw_pointer_write_unaligned", &output)?;

        assert_eq!(
            card.operation.family,
            OperationFamily::RawPointerWriteUnaligned
        );
        assert!(!card.hazards.contains(&HazardKind::Alignment));
        assert!(
            card.obligations
                .iter()
                .all(|obligation| obligation.key != "alignment")
        );
        assert!(card.contract.present);
        assert!(
            obligation_discharge_present(card, "bounds"),
            "length evidence should still satisfy the bounds obligation"
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        Ok(())
    }

    #[test]
    fn raw_pointer_write_maybeuninit_evidence_requires_target_context() -> Result<(), String> {
        let output = fixture_output("raw_pointer_write_other_maybeuninit_not_guard")?;
        let card = single_card("raw_pointer_write_other_maybeuninit_not_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "initialized"));
        assert!(!obligation_discharge_present(card, "bounds"));
        Ok(())
    }

    #[test]
    fn raw_pointer_write_u8_evidence_requires_target_pointer() -> Result<(), String> {
        let output = fixture_output("raw_pointer_write_other_u8_not_guard")?;
        let card = single_card("raw_pointer_write_other_u8_not_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "initialized"));
        assert!(!obligation_discharge_present(card, "alignment"));
        Ok(())
    }

    #[test]
    fn raw_pointer_write_bool_bytes_guard_discharges_byte_obligations() -> Result<(), String> {
        for fixture in [
            "raw_pointer_write_bool_bytes_guard",
            "raw_pointer_write_bool_conjunct_branch_guard",
            "raw_pointer_write_bool_disjunct_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "initialized"));
            assert!(obligation_discharge_present(card, "alignment"));
            assert!(!obligation_discharge_present(card, "pointer-live"));
            assert!(!obligation_discharge_present(card, "bounds"));
            assert!(!obligation_discharge_present(card, "allocation"));
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_write_bool_bytes_guard_requires_fresh_byte() -> Result<(), String> {
        let output = fixture_output("raw_pointer_write_bool_reassigned_byte_not_guard")?;
        let card = single_card("raw_pointer_write_bool_reassigned_byte_not_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(obligation_discharge_present(card, "alignment"));
        assert!(!obligation_discharge_present(card, "initialized"));
        assert!(!obligation_discharge_present(card, "pointer-live"));
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(!obligation_discharge_present(card, "allocation"));
        Ok(())
    }

    #[test]
    fn raw_pointer_write_bool_bytes_guard_requires_open_branch() -> Result<(), String> {
        for fixture in [
            "raw_pointer_write_bool_closed_branch_not_guard",
            "raw_pointer_write_bool_disjunct_branch_not_guard",
            "raw_pointer_write_bool_conjunct_return_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "alignment"));
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(!obligation_discharge_present(card, "pointer-live"));
            assert!(!obligation_discharge_present(card, "bounds"));
            assert!(!obligation_discharge_present(card, "allocation"));
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_write_bounds_evidence_requires_current_operation() -> Result<(), String> {
        let output = fixture_output("raw_pointer_write_previous_slice_not_guard")?;
        let card = single_card("raw_pointer_write_previous_slice_not_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(!obligation_discharge_present(card, "initialized"));
        Ok(())
    }

    #[test]
    fn raw_pointer_write_target_evidence_requires_current_operation() -> Result<(), String> {
        for fixture in [
            "raw_pointer_write_previous_u8_not_guard",
            "raw_pointer_write_previous_bool_not_guard",
            "raw_pointer_write_previous_maybeuninit_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerWrite);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "alignment"));
            assert!(!obligation_discharge_present(card, "initialized"));
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_v1_evidence_stays_obligation_specific() -> Result<(), String> {
        for fixture in [
            "raw_pointer_alignment",
            "align_of_only_not_guard",
            "alignment_other_pointer_not_guard",
            "raw_pointer_alignment_post_check_not_guard",
            "raw_pointer_alignment_observed_not_guard",
            "raw_pointer_alignment_closed_branch_not_guard",
            "raw_pointer_alignment_reassigned_pointer_not_guard",
            "raw_pointer_alignment_modulo_observed_not_guard",
            "raw_pointer_alignment_modulo_closed_branch_not_guard",
            "raw_pointer_alignment_modulo_reassigned_pointer_not_guard",
            "comment_alignment_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                obligation_discharge_present(card, "bounds"),
                "{fixture} should keep length or bounds evidence"
            );
            assert!(
                !obligation_discharge_present(card, "alignment"),
                "{fixture} should not let comments or length checks discharge alignment"
            );
            assert!(!card.discharge.present);
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_read_bounds_evidence_rejects_unenforced_guards() -> Result<(), String> {
        for fixture in [
            "raw_pointer_read_bounds_observed_not_guard",
            "raw_pointer_read_len_capacity_observed_not_guard",
            "raw_pointer_read_assert_shadowed_origin_not_guard",
            "raw_pointer_read_len_capacity_assert_shadowed_origin_not_guard",
            "raw_pointer_read_open_branch_shadowed_origin_not_guard",
            "raw_pointer_read_typed_shadowed_origin_not_guard",
            "raw_pointer_read_other_len_not_guard",
            "raw_pointer_read_reassigned_origin_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "bounds"));
            assert!(!obligation_discharge_present(card, "alignment"));
            assert!(!card.discharge.present);
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_read_bounds_evidence_accepts_open_same_origin_branch() -> Result<(), String> {
        let output = fixture_output("raw_pointer_read_open_branch_bounds_guard")?;
        let card = single_card("raw_pointer_read_open_branch_bounds_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(obligation_discharge_present(card, "bounds"));
        assert!(!obligation_discharge_present(card, "alignment"));
        Ok(())
    }

    #[test]
    fn raw_pointer_read_bounds_evidence_accepts_cast_pointer_origin() -> Result<(), String> {
        let output = fixture_output("raw_pointer_read_cast_origin_bounds_guard")?;
        let card = single_card("raw_pointer_read_cast_origin_bounds_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(obligation_discharge_present(card, "bounds"));
        assert!(!obligation_discharge_present(card, "alignment"));
        Ok(())
    }

    #[test]
    fn raw_pointer_read_bounds_evidence_accepts_as_cast_pointer_origin() -> Result<(), String> {
        let output = fixture_output("raw_pointer_read_as_cast_origin_bounds_guard")?;
        let card = single_card("raw_pointer_read_as_cast_origin_bounds_guard", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(obligation_discharge_present(card, "bounds"));
        assert!(!obligation_discharge_present(card, "alignment"));
        Ok(())
    }

    #[test]
    fn raw_pointer_alignment_evidence_accepts_enforced_same_pointer_guard() -> Result<(), String> {
        for fixture in [
            "raw_pointer_alignment_is_aligned_guard",
            "raw_pointer_alignment_modulo_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
            assert!(obligation_discharge_present(card, "bounds"));
            assert!(obligation_discharge_present(card, "alignment"));
            assert!(!obligation_discharge_present(card, "pointer-live"));
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(!obligation_discharge_present(card, "allocation"));
        }
        Ok(())
    }

    #[test]
    fn raw_pointer_v1_negative_cases_stay_pinned() -> Result<(), String> {
        let safe_reference = fixture_output("safe_reference_deref_no_cards")?;
        assert_eq!(safe_reference.summary.cards, 0);
        assert!(safe_reference.cards.is_empty());

        let split_unknown = fixture_output("split_unsafe_block")?;
        let card = single_card("split_unsafe_block", &split_unknown)?;
        assert_eq!(card.site.kind, UnsafeSiteKind::UnsafeBlock);
        assert_eq!(card.operation.family, OperationFamily::Unknown);
        assert_eq!(card.class, ReviewClass::ContractMissing);
        Ok(())
    }

    #[test]
    fn specialist_review_classes_get_routed_next_actions() {
        for (class, expected) in [
            (ReviewClass::RequiresSanitizer, "sanitizer"),
            (ReviewClass::RequiresKaniOrCrux, "Kani/Crux"),
            (ReviewClass::ReachableUnwitnessed, "witness receipt"),
            (ReviewClass::WitnessMismatch, "matching receipt"),
            (ReviewClass::StaticUnknown, "witness route"),
        ] {
            let summary = next_action_summary(&class, "raw_pointer_read", false, &[]);
            assert!(
                summary.contains(expected),
                "`{}` next action `{summary}` should mention `{expected}`",
                class.as_str()
            );
        }
    }

    #[test]
    fn guarded_cards_get_route_specific_witness_next_actions() {
        let human_route = [test_witness_route(WitnessKind::HumanDeepReview)];
        let summary = next_action_summary(
            &ReviewClass::GuardedUnwitnessed,
            "unknown",
            false,
            &human_route,
        );
        assert!(
            summary.contains("human deep-review witness receipt"),
            "human-routed guarded card next action `{summary}` should route to human deep-review receipt evidence"
        );
        assert!(summary.contains("static limitation"));

        let miri_careful_routes = [
            test_witness_route(WitnessKind::Miri),
            test_witness_route(WitnessKind::CargoCareful),
        ];
        let miri_supported = next_action_summary(
            &ReviewClass::GuardedUnwitnessed,
            "raw_pointer_read",
            false,
            &miri_careful_routes,
        );
        assert!(miri_supported.contains("Miri"));
        assert!(miri_supported.contains("cargo-careful"));
        assert!(miri_supported.contains("witness receipt"));
        assert!(!miri_supported.contains("human deep-review"));
    }

    #[test]
    fn inline_asm_guard_missing_next_action_routes_to_manual_invariant_review() {
        let human_route = [test_witness_route(WitnessKind::HumanDeepReview)];
        let summary = next_action_summary(
            &ReviewClass::GuardMissing,
            "inline_asm",
            false,
            &human_route,
        );

        assert!(summary.contains("inline_asm"));
        assert!(summary.contains("manually"));
        assert!(summary.contains("guard evidence"));
        assert!(summary.contains("human deep-review receipt"));
        assert!(!summary.contains("local guard that discharges"));
    }

    #[test]
    fn pin_unchecked_guard_missing_next_action_routes_to_manual_invariant_review() {
        let human_route = [test_witness_route(WitnessKind::HumanDeepReview)];
        let summary = next_action_summary(
            &ReviewClass::GuardMissing,
            "pin_unchecked",
            false,
            &human_route,
        );

        assert!(summary.contains("pin_unchecked"));
        assert!(summary.contains("move-prevention"));
        assert!(summary.contains("projection invariants"));
        assert!(summary.contains("guard evidence"));
        assert!(summary.contains("human deep-review receipt"));
        assert!(!summary.contains("local guard that discharges"));
    }

    #[test]
    fn unsafe_fn_call_guard_missing_next_action_routes_to_callee_contract_review() {
        let human_route = [test_witness_route(WitnessKind::HumanDeepReview)];
        let summary = next_action_summary(
            &ReviewClass::GuardMissing,
            "unsafe_fn_call",
            false,
            &human_route,
        );

        assert!(summary.contains("unsafe_fn_call"));
        assert!(summary.contains("callee contract"));
        assert!(summary.contains("obligation-specific guard evidence"));
        assert!(!summary.contains("local guard that discharges"));
    }

    #[test]
    fn capped_repo_scan_prefers_source_roots_before_miscellaneous_rust_files() -> Result<(), String>
    {
        let root = unique_temp_dir("unsafe-review-capped-repo")?;
        fs::create_dir_all(root.join("benchmarks/haystacks/code"))
            .map_err(|err| format!("create benchmark dirs failed: {err}"))?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"capped-repo-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(
            root.join("benchmarks/haystacks/code/rust-library.rs"),
            "pub unsafe fn fixture_data() {}\n",
        )
        .map_err(|err| format!("write benchmark file failed: {err}"))?;
        fs::write(root.join("src/lib.rs"), "pub unsafe fn source_root() {}\n")
            .map_err(|err| format!("write src file failed: {err}"))?;

        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: Some(1),
        })?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(output.cards.len(), 1);
        assert_eq!(output.summary.cards, 1);
        assert_eq!(
            output.cards[0].site.location.file,
            PathBuf::from("src/lib.rs")
        );
        assert_eq!(output.cards[0].site.owner, Some("source_root".to_string()));
        Ok(())
    }

    #[test]
    fn repo_scan_honors_discovery_filters_before_analysis() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-filtered-repo")?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::create_dir_all(root.join("packages/pkg/src"))
            .map_err(|err| format!("create package src failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"filtered-repo-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(root.join("src/lib.rs"), "pub unsafe fn selected() {}\n")
            .map_err(|err| format!("write src file failed: {err}"))?;
        fs::write(
            root.join("packages/pkg/src/lib.rs"),
            "pub unsafe fn excluded() {}\n",
        )
        .map_err(|err| format!("write package file failed: {err}"))?;

        let output = analyze_with_discovery(
            AnalyzeInput {
                root: root.clone(),
                scope: Scope::Repo,
                diff: DiffSource::NoneRepoScan,
                mode: AnalysisMode::Repo,
                policy: PolicyMode::Advisory,
                include_unchanged_tests: true,
                max_cards: None,
            },
            DiscoveryOptions {
                include: vec!["src/**/*.rs".to_string(), "packages/**/*.rs".to_string()],
                exclude: vec!["packages/**".to_string()],
                ..DiscoveryOptions::repo_defaults()
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(output.summary.rust_files, 1);
        assert_eq!(output.summary.changed_rust_files, 1);
        assert_eq!(output.cards.len(), 1);
        assert_eq!(
            output.cards[0].site.location.file,
            PathBuf::from("src/lib.rs")
        );
        Ok(())
    }

    #[test]
    fn repo_scan_status_reports_discovery_scanning_and_completion() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-repo-status")?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"repo-status-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            "pub unsafe fn read_header(ptr: *const u8) -> u32 { unsafe { ptr.cast::<u32>().read() } }\n",
        )
        .map_err(|err| format!("write src file failed: {err}"))?;

        let mut statuses = Vec::new();
        let output = analyze_with_discovery_and_progress(
            AnalyzeInput {
                root: root.clone(),
                scope: Scope::Repo,
                diff: DiffSource::NoneRepoScan,
                mode: AnalysisMode::Repo,
                policy: PolicyMode::Advisory,
                include_unchanged_tests: true,
                max_cards: None,
            },
            DiscoveryOptions::repo_defaults(),
            |status| {
                statuses.push(status.clone());
                Ok(())
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert!(
            statuses
                .iter()
                .any(|status| status.phase == RepoScanPhase::Discovering)
        );
        assert!(
            statuses
                .iter()
                .any(|status| status.phase == RepoScanPhase::Scanning)
        );
        let complete = statuses
            .last()
            .ok_or_else(|| "expected at least one repo scan status".to_string())?;
        assert_eq!(complete.phase, RepoScanPhase::Complete);
        assert!(complete.completed);
        assert_eq!(complete.files_discovered, 1);
        assert_eq!(complete.files_scanned, 1);
        assert_eq!(complete.cards_found, output.cards.len());
        assert_eq!(complete.last_path, Some(PathBuf::from("src/lib.rs")));
        Ok(())
    }

    #[test]
    fn repo_scan_events_include_completed_file_snapshots() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-repo-events")?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"repo-event-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(root.join("src/lib.rs"), "pub unsafe fn source_root() {}\n")
            .map_err(|err| format!("write src file failed: {err}"))?;
        fs::write(root.join("src/z.rs"), "pub fn safe() {}\n")
            .map_err(|err| format!("write safe source failed: {err}"))?;

        let mut partials = Vec::new();
        let output = analyze_with_discovery_and_repo_events(
            AnalyzeInput {
                root: root.clone(),
                scope: Scope::Repo,
                diff: DiffSource::NoneRepoScan,
                mode: AnalysisMode::Repo,
                policy: PolicyMode::Advisory,
                include_unchanged_tests: true,
                max_cards: Some(1),
            },
            DiscoveryOptions::repo_defaults(),
            |event| {
                if let Some(partial) = &event.partial_output
                    && event.status.phase == RepoScanPhase::Scanning
                    && event.status.files_scanned == 1
                {
                    partials.push(partial.clone());
                }
                Ok(())
            },
        )?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let first_partial = partials
            .first()
            .ok_or_else(|| "expected a partial snapshot after the first file".to_string())?;
        assert_eq!(first_partial.summary.rust_files, 2);
        assert_eq!(first_partial.summary.changed_rust_files, 2);
        assert_eq!(first_partial.cards.len(), 1);
        assert_eq!(
            first_partial.cards[0].site.location.file,
            PathBuf::from("src/lib.rs")
        );
        assert_eq!(output.cards.len(), 1);
        Ok(())
    }

    #[test]
    fn operation_cards_inside_unsafe_fn_inherit_owner_safety_docs() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-owner-contract")?;
        fs::create_dir_all(root.join("src")).map_err(|err| format!("create src failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"owner-contract-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .map_err(|err| format!("write Cargo.toml failed: {err}"))?;
        fs::write(
            root.join("src/lib.rs"),
            r#"/// Advances a raw pointer.
///
/// # Safety
///
/// Caller must ensure `ptr.add(offset)` remains within the same allocation.
pub unsafe fn advance(ptr: *const u8, offset: usize) -> *const u8 {
    let _padding0 = offset;
    let _padding1 = offset;
    let _padding2 = offset;
    let _padding3 = offset;
    let _padding4 = offset;
    let _padding5 = offset;
    let _padding6 = offset;
    unsafe { ptr.add(offset) }
}
"#,
        )
        .map_err(|err| format!("write src file failed: {err}"))?;

        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let Some(card) = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::PointerArithmetic)
        else {
            return Err(format!(
                "expected pointer arithmetic card: {:#?}",
                output.cards
            ));
        };
        assert_eq!(card.site.owner, Some("advance".to_string()));
        assert!(
            card.contract.present,
            "operation card should inherit enclosing unsafe fn # Safety docs"
        );
        Ok(())
    }

    #[test]
    fn public_unsafe_api_contract_evidence_requires_safety_docs() -> Result<(), String> {
        for fixture in [
            "public_unsafe_fn_missing_safety",
            "public_unsafe_trait_missing_safety",
            "public_unsafe_fn_safety_comment_not_docs",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.class, ReviewClass::ContractMissing);
            assert!(card.site.public_api_surface);
            assert!(!card.contract.present);
            assert!(
                card.contract.summary.contains("# Safety"),
                "{fixture} should ask for public safety docs"
            );
            assert!(
                card.missing.iter().any(|missing| missing.kind == "contract"
                    && missing.message.contains("public `# Safety`")),
                "{fixture} should not accept local SAFETY prose as public API docs"
            );
            assert!(
                card.next_action.summary.contains("public `# Safety`")
                    && !card.next_action.summary.contains("SAFETY:"),
                "{fixture} next action should require public safety docs, not a local SAFETY comment"
            );
            assert!(
                card.site.owner.is_some(),
                "{fixture} should preserve the public API owner in the card"
            );
        }
        Ok(())
    }

    #[test]
    fn documented_public_unsafe_api_does_not_require_local_guard() -> Result<(), String> {
        for fixture in [
            "public_unsafe_fn_with_safety_docs",
            "public_unsafe_fn_safety_colon_docs",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.site.public_api_surface);
            assert!(card.contract.present);
            assert!(card.discharge.present);
            assert!(
                !card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} should not ask for local declaration guard evidence"
            );
            assert!(
                card.missing.iter().any(|missing| missing.kind == "witness"),
                "{fixture} should still preserve witness prompts"
            );
        }
        Ok(())
    }

    #[test]
    fn documented_private_unsafe_fn_does_not_require_local_guard() -> Result<(), String> {
        let output = fixture_output("documented_private_unsafe_fn")?;
        let card = single_card("documented_private_unsafe_fn", &output)?;

        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(!card.site.public_api_surface);
        assert!(card.contract.present);
        assert!(card.discharge.present);
        assert!(
            !card.missing.iter().any(|missing| missing.kind == "guard"),
            "documented private unsafe declarations should not ask for local declaration guard evidence"
        );
        assert!(
            card.missing.iter().any(|missing| missing.kind == "witness"),
            "documented private unsafe declarations should still preserve witness prompts"
        );
        Ok(())
    }

    #[test]
    fn unsafe_call_wrapper_uses_concrete_operation_family() -> Result<(), String> {
        let output = fixture_output("unsafe_fn_call_wrapper")?;
        let card = single_card("unsafe_fn_call_wrapper", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnsafeFnCall);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.id.0.contains("encode-utf8"));
        Ok(())
    }

    #[test]
    fn multiline_unsafe_call_wrapper_uses_concrete_operation_family() -> Result<(), String> {
        let output = fixture_output("multiline_unsafe_fn_call_wrapper")?;
        let card = single_card("multiline_unsafe_fn_call_wrapper", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnsafeFnCall);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.id.0.contains("encode-utf8"));
        Ok(())
    }

    #[test]
    fn unsafe_call_path_prefers_inner_unchecked_constructor_callee() {
        assert_eq!(
            unsafe_call_path("unsafe { Some(One::new_unchecked(needle)) }"),
            "new_unchecked"
        );
        assert_eq!(
            unsafe_call_path("unsafe { Some(One::new_unchecked::<Needle>(needle)) }"),
            "new_unchecked"
        );
        assert_eq!(
            unsafe_call_path("unsafe { self.reserve_rehash(hasher) }"),
            "reserve_rehash"
        );
    }

    #[test]
    fn unsafe_call_path_strips_trailing_turbofish_from_callee() {
        assert_eq!(
            unsafe_call_path("unsafe { crate::ffi::call_unchecked::<Header>(ptr) }"),
            "call_unchecked"
        );
        assert_eq!(
            unsafe_call_path("unsafe { self.call_unchecked::<Header>(ptr) }"),
            "call_unchecked"
        );
        assert_eq!(
            unsafe_call_path("unsafe { call_unchecked::<Header>(ptr) }"),
            "call_unchecked"
        );
        assert_eq!(
            unsafe_call_path("unsafe { crate::ffi::call_unchecked::<Option<Header>>(ptr) }"),
            "call_unchecked"
        );
    }

    #[test]
    fn unchecked_constructor_availability_guard_is_unsafe_call_evidence() -> Result<(), String> {
        for fixture in [
            "unchecked_constructor_availability_guard",
            "unchecked_constructor_availability_assert_guard",
            "unchecked_constructor_unavailable_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnsafeFnCall);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.discharge.present);
            assert!(obligation_discharge_present(card, "callee-contract"));
            assert!(card.site.snippet.contains("new_unchecked"));
            assert!(card.id.0.contains("new-unchecked"));
            assert!(!card.id.0.contains("-some-"));
        }
        Ok(())
    }

    #[test]
    fn unchecked_constructor_availability_guard_requires_same_receiver() -> Result<(), String> {
        for fixture in [
            "unchecked_constructor_other_availability_not_guard",
            "unchecked_constructor_availability_observed_not_guard",
            "unchecked_constructor_availability_closed_branch_not_guard",
            "unchecked_constructor_unavailable_return_comment_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnsafeFnCall);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "callee-contract"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must not resolve this card's guard prompt"
            );
            assert!(card.site.snippet.contains("One::new_unchecked"));
        }
        Ok(())
    }

    #[test]
    fn nonnull_new_guard_discharges_non_null_obligation() -> Result<(), String> {
        let output = fixture_output("nonnull_new_guard")?;
        let card = single_card("nonnull_new_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.discharge.present);
        assert!(obligation_discharge_present(card, "non-null"));
        assert!(
            card.missing.iter().all(|missing| missing.kind != "guard"),
            "NonNull::new guard evidence should resolve the local guard prompt"
        );
        assert!(card.id.0.contains("nonnull-unchecked"));
        Ok(())
    }

    #[test]
    fn nonnull_new_guard_variants_require_same_pointer_applicability() -> Result<(), String> {
        for fixture in [
            "nonnull_new_guard",
            "nonnull_if_let_new_guard",
            "nonnull_let_else_new_guard",
            "nonnull_match_new_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(card.discharge.present);
            assert!(obligation_discharge_present(card, "non-null"));
            assert!(
                card.missing.iter().all(|missing| missing.kind != "guard"),
                "{fixture} should resolve the local guard prompt with same-pointer evidence"
            );
            assert!(card.id.0.contains("nonnull-unchecked"));
        }
        Ok(())
    }

    #[test]
    fn nonnull_new_guard_for_other_pointer_is_not_evidence() -> Result<(), String> {
        let output = fixture_output("nonnull_other_guard_not_evidence")?;
        let card = single_card("nonnull_other_guard_not_evidence", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "non-null"));
        assert!(
            card.missing.iter().any(|missing| missing.kind == "guard"),
            "checking a different pointer must not resolve this card's guard prompt"
        );
        assert!(card.id.0.contains("nonnull-unchecked"));
        Ok(())
    }

    #[test]
    fn nonnull_is_null_branch_must_exit_to_count_as_guard() -> Result<(), String> {
        for fixture in [
            "nonnull_is_null_nonreturning_not_guard",
            "nonnull_is_null_return_comment_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "non-null"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture}: observing null without exiting must not resolve this card's guard prompt"
            );
            assert!(card.id.0.contains("nonnull-unchecked"));
        }
        Ok(())
    }

    #[test]
    fn nonnull_new_observation_is_not_guard_evidence() -> Result<(), String> {
        let output = fixture_output("nonnull_observed_not_guard")?;
        let card = single_card("nonnull_observed_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "non-null"));
        assert!(
            card.missing.iter().any(|missing| missing.kind == "guard"),
            "observing NonNull::new without checking the result must not resolve this card's guard prompt"
        );
        assert!(card.id.0.contains("nonnull-unchecked"));
        Ok(())
    }

    #[test]
    fn nonnull_post_check_is_not_guard_evidence() -> Result<(), String> {
        let output = fixture_output("nonnull_post_check_not_guard")?;
        let card = single_card("nonnull_post_check_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "non-null"));
        assert!(
            card.missing.iter().any(|missing| missing.kind == "guard"),
            "checking the pointer after NonNull::new_unchecked must not resolve this card's guard prompt"
        );
        assert!(card.id.0.contains("new-unchecked"));
        Ok(())
    }

    #[test]
    fn nonnull_pointer_evidence_rejects_stale_reassignment_shadowing_or_cast_target()
    -> Result<(), String> {
        for fixture in [
            "nonnull_new_reassigned_ptr_not_guard",
            "nonnull_new_shadowed_ptr_not_guard",
            "nonnull_cast_checked_pointer_not_guard",
            "nonnull_method_receiver_reassigned_not_guard",
            "nonnull_method_receiver_shadowed_not_guard",
            "nonnull_is_null_reassigned_ptr_not_guard",
            "nonnull_is_null_shadowed_ptr_not_guard",
            "nonnull_is_null_open_branch_shadowed_ptr_not_guard",
            "nonnull_if_let_new_reassigned_ptr_not_guard",
            "nonnull_if_let_new_shadowed_ptr_not_guard",
            "nonnull_let_else_new_reassigned_ptr_not_guard",
            "nonnull_let_else_new_shadowed_ptr_not_guard",
            "nonnull_match_new_reassigned_ptr_not_guard",
            "nonnull_match_new_shadowed_ptr_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!card.discharge.present);
            assert!(!obligation_discharge_present(card, "non-null"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must keep stale pointer evidence from resolving the guard prompt"
            );
            assert!(card.id.0.contains("nonnull-unchecked"));
        }
        Ok(())
    }

    #[test]
    fn nested_unsafe_operation_does_not_emit_parent_duplicate() -> Result<(), String> {
        let output = fixture_output("nested_unsafe_operation_call_dedupe")?;
        let card = single_card("nested_unsafe_operation_call_dedupe", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::NonNullUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "non-null"));
        assert!(card.id.0.contains("nonnull-unchecked"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_uses_bounds_operation_family() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_bounds")?;
        let card = single_card("get_unchecked_mut_bounds", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::Bounds));
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("get-unchecked-mut"));
        Ok(())
    }

    #[test]
    fn get_unchecked_mut_bounds_evidence_requires_same_receiver() -> Result<(), String> {
        let guarded = fixture_output("get_unchecked_mut_len_guard")?;
        let guarded_card = single_card("get_unchecked_mut_len_guard", &guarded)?;
        assert_eq!(guarded_card.operation.family, OperationFamily::GetUnchecked);
        assert_eq!(guarded_card.class, ReviewClass::GuardedUnwitnessed);
        assert!(obligation_discharge_present(guarded_card, "bounds"));

        let other_guard = fixture_output("get_unchecked_mut_other_len_not_guard")?;
        let other_guard_card = single_card("get_unchecked_mut_other_len_not_guard", &other_guard)?;
        assert_eq!(
            other_guard_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(other_guard_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(other_guard_card, "bounds"));

        let post_check = fixture_output("get_unchecked_mut_post_check_not_guard")?;
        let post_check_card = single_card("get_unchecked_mut_post_check_not_guard", &post_check)?;
        assert_eq!(
            post_check_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(post_check_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(post_check_card, "bounds"));

        let observed_bounds = fixture_output("get_unchecked_mut_bounds_observed_not_guard")?;
        let observed_bounds_card = single_card(
            "get_unchecked_mut_bounds_observed_not_guard",
            &observed_bounds,
        )?;
        assert_eq!(
            observed_bounds_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(observed_bounds_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            observed_bounds_card,
            "bounds"
        ));

        let closed_bounds = fixture_output("get_unchecked_mut_closed_bounds_not_guard")?;
        let closed_bounds_card =
            single_card("get_unchecked_mut_closed_bounds_not_guard", &closed_bounds)?;
        assert_eq!(
            closed_bounds_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(closed_bounds_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(closed_bounds_card, "bounds"));

        let reassigned_index = fixture_output("get_unchecked_mut_reassigned_index_not_guard")?;
        let reassigned_index_card = single_card(
            "get_unchecked_mut_reassigned_index_not_guard",
            &reassigned_index,
        )?;
        assert_eq!(
            reassigned_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(reassigned_index_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            reassigned_index_card,
            "bounds"
        ));

        let compound_reassigned_index =
            fixture_output("get_unchecked_mut_compound_reassigned_index_not_guard")?;
        let compound_reassigned_index_card = single_card(
            "get_unchecked_mut_compound_reassigned_index_not_guard",
            &compound_reassigned_index,
        )?;
        assert_eq!(
            compound_reassigned_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            compound_reassigned_index_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            compound_reassigned_index_card,
            "bounds"
        ));

        let shadowed_index = fixture_output("get_unchecked_mut_shadowed_index_not_guard")?;
        let shadowed_index_card = single_card(
            "get_unchecked_mut_shadowed_index_not_guard",
            &shadowed_index,
        )?;
        assert_eq!(
            shadowed_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(shadowed_index_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(shadowed_index_card, "bounds"));

        let shadowed_probe_index =
            fixture_output("get_unchecked_mut_get_probe_shadowed_index_not_guard")?;
        let shadowed_probe_index_card = single_card(
            "get_unchecked_mut_get_probe_shadowed_index_not_guard",
            &shadowed_probe_index,
        )?;
        assert_eq!(
            shadowed_probe_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(shadowed_probe_index_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            shadowed_probe_index_card,
            "bounds"
        ));

        let reassigned_receiver =
            fixture_output("get_unchecked_mut_reassigned_receiver_not_guard")?;
        let reassigned_receiver_card = single_card(
            "get_unchecked_mut_reassigned_receiver_not_guard",
            &reassigned_receiver,
        )?;
        assert_eq!(
            reassigned_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(reassigned_receiver_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            reassigned_receiver_card,
            "bounds"
        ));

        let reassigned_receiver_path =
            fixture_output("get_unchecked_mut_reassigned_receiver_path_not_guard")?;
        let reassigned_receiver_path_card = single_card(
            "get_unchecked_mut_reassigned_receiver_path_not_guard",
            &reassigned_receiver_path,
        )?;
        assert_eq!(
            reassigned_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            reassigned_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            reassigned_receiver_path_card,
            "bounds"
        ));

        let shadowed_receiver = fixture_output("get_unchecked_mut_shadowed_receiver_not_guard")?;
        let shadowed_receiver_card = single_card(
            "get_unchecked_mut_shadowed_receiver_not_guard",
            &shadowed_receiver,
        )?;
        assert_eq!(
            shadowed_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(shadowed_receiver_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            shadowed_receiver_card,
            "bounds"
        ));

        let reassigned_probe_receiver =
            fixture_output("get_unchecked_mut_get_probe_reassigned_receiver_not_guard")?;
        let reassigned_probe_receiver_card = single_card(
            "get_unchecked_mut_get_probe_reassigned_receiver_not_guard",
            &reassigned_probe_receiver,
        )?;
        assert_eq!(
            reassigned_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            reassigned_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            reassigned_probe_receiver_card,
            "bounds"
        ));

        let reassigned_probe_receiver_path =
            fixture_output("get_unchecked_mut_get_probe_reassigned_receiver_path_not_guard")?;
        let reassigned_probe_receiver_path_card = single_card(
            "get_unchecked_mut_get_probe_reassigned_receiver_path_not_guard",
            &reassigned_probe_receiver_path,
        )?;
        assert_eq!(
            reassigned_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            reassigned_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            reassigned_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_probe_receiver_path =
            fixture_output("get_unchecked_mut_get_probe_shadowed_receiver_path_not_guard")?;
        let shadowed_probe_receiver_path_card = single_card(
            "get_unchecked_mut_get_probe_shadowed_receiver_path_not_guard",
            &shadowed_probe_receiver_path,
        )?;
        assert_eq!(
            shadowed_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_probe_receiver =
            fixture_output("get_unchecked_mut_get_probe_shadowed_receiver_not_guard")?;
        let shadowed_probe_receiver_card = single_card(
            "get_unchecked_mut_get_probe_shadowed_receiver_not_guard",
            &shadowed_probe_receiver,
        )?;
        assert_eq!(
            shadowed_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_probe_receiver_card,
            "bounds"
        ));

        let stale_return_probe_receiver = fixture_output(
            "get_unchecked_mut_get_probe_early_return_reassigned_receiver_not_guard",
        )?;
        let stale_return_probe_receiver_card = single_card(
            "get_unchecked_mut_get_probe_early_return_reassigned_receiver_not_guard",
            &stale_return_probe_receiver,
        )?;
        assert_eq!(
            stale_return_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_return_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_return_probe_receiver_card,
            "bounds"
        ));

        let shadowed_return_probe_index =
            fixture_output("get_unchecked_mut_get_probe_early_return_shadowed_index_not_guard")?;
        let shadowed_return_probe_index_card = single_card(
            "get_unchecked_mut_get_probe_early_return_shadowed_index_not_guard",
            &shadowed_return_probe_index,
        )?;
        assert_eq!(
            shadowed_return_probe_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_return_probe_index_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_return_probe_index_card,
            "bounds"
        ));

        let shadowed_return_probe_receiver =
            fixture_output("get_unchecked_mut_get_probe_early_return_shadowed_receiver_not_guard")?;
        let shadowed_return_probe_receiver_card = single_card(
            "get_unchecked_mut_get_probe_early_return_shadowed_receiver_not_guard",
            &shadowed_return_probe_receiver,
        )?;
        assert_eq!(
            shadowed_return_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_return_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_return_probe_receiver_card,
            "bounds"
        ));

        let shadowed_if_let_probe_index =
            fixture_output("get_unchecked_mut_if_let_get_shadowed_index_not_guard")?;
        let shadowed_if_let_probe_index_card = single_card(
            "get_unchecked_mut_if_let_get_shadowed_index_not_guard",
            &shadowed_if_let_probe_index,
        )?;
        assert_eq!(
            shadowed_if_let_probe_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_if_let_probe_index_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_if_let_probe_index_card,
            "bounds"
        ));

        let stale_if_let_probe_receiver =
            fixture_output("get_unchecked_mut_if_let_get_reassigned_receiver_not_guard")?;
        let stale_if_let_probe_receiver_card = single_card(
            "get_unchecked_mut_if_let_get_reassigned_receiver_not_guard",
            &stale_if_let_probe_receiver,
        )?;
        assert_eq!(
            stale_if_let_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_if_let_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_if_let_probe_receiver_card,
            "bounds"
        ));

        let stale_if_let_probe_receiver_path =
            fixture_output("get_unchecked_mut_if_let_get_reassigned_receiver_path_not_guard")?;
        let stale_if_let_probe_receiver_path_card = single_card(
            "get_unchecked_mut_if_let_get_reassigned_receiver_path_not_guard",
            &stale_if_let_probe_receiver_path,
        )?;
        assert_eq!(
            stale_if_let_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_if_let_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_if_let_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_if_let_probe_receiver_path =
            fixture_output("get_unchecked_mut_if_let_get_shadowed_receiver_path_not_guard")?;
        let shadowed_if_let_probe_receiver_path_card = single_card(
            "get_unchecked_mut_if_let_get_shadowed_receiver_path_not_guard",
            &shadowed_if_let_probe_receiver_path,
        )?;
        assert_eq!(
            shadowed_if_let_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_if_let_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_if_let_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_if_let_probe_receiver =
            fixture_output("get_unchecked_mut_if_let_get_shadowed_receiver_not_guard")?;
        let shadowed_if_let_probe_receiver_card = single_card(
            "get_unchecked_mut_if_let_get_shadowed_receiver_not_guard",
            &shadowed_if_let_probe_receiver,
        )?;
        assert_eq!(
            shadowed_if_let_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_if_let_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_if_let_probe_receiver_card,
            "bounds"
        ));

        let shadowed_let_else_probe_index =
            fixture_output("get_unchecked_mut_let_else_get_shadowed_index_not_guard")?;
        let shadowed_let_else_probe_index_card = single_card(
            "get_unchecked_mut_let_else_get_shadowed_index_not_guard",
            &shadowed_let_else_probe_index,
        )?;
        assert_eq!(
            shadowed_let_else_probe_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_let_else_probe_index_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_let_else_probe_index_card,
            "bounds"
        ));

        let stale_let_else_probe_receiver =
            fixture_output("get_unchecked_mut_let_else_get_reassigned_receiver_not_guard")?;
        let stale_let_else_probe_receiver_card = single_card(
            "get_unchecked_mut_let_else_get_reassigned_receiver_not_guard",
            &stale_let_else_probe_receiver,
        )?;
        assert_eq!(
            stale_let_else_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_let_else_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_let_else_probe_receiver_card,
            "bounds"
        ));

        let stale_let_else_probe_receiver_path =
            fixture_output("get_unchecked_mut_let_else_get_reassigned_receiver_path_not_guard")?;
        let stale_let_else_probe_receiver_path_card = single_card(
            "get_unchecked_mut_let_else_get_reassigned_receiver_path_not_guard",
            &stale_let_else_probe_receiver_path,
        )?;
        assert_eq!(
            stale_let_else_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_let_else_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_let_else_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_let_else_probe_receiver_path =
            fixture_output("get_unchecked_mut_let_else_get_shadowed_receiver_path_not_guard")?;
        let shadowed_let_else_probe_receiver_path_card = single_card(
            "get_unchecked_mut_let_else_get_shadowed_receiver_path_not_guard",
            &shadowed_let_else_probe_receiver_path,
        )?;
        assert_eq!(
            shadowed_let_else_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_let_else_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_let_else_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_let_else_probe_receiver =
            fixture_output("get_unchecked_mut_let_else_get_shadowed_receiver_not_guard")?;
        let shadowed_let_else_probe_receiver_card = single_card(
            "get_unchecked_mut_let_else_get_shadowed_receiver_not_guard",
            &shadowed_let_else_probe_receiver,
        )?;
        assert_eq!(
            shadowed_let_else_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_let_else_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_let_else_probe_receiver_card,
            "bounds"
        ));

        let shadowed_match_probe_index =
            fixture_output("get_unchecked_mut_match_get_shadowed_index_not_guard")?;
        let shadowed_match_probe_index_card = single_card(
            "get_unchecked_mut_match_get_shadowed_index_not_guard",
            &shadowed_match_probe_index,
        )?;
        assert_eq!(
            shadowed_match_probe_index_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_match_probe_index_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_match_probe_index_card,
            "bounds"
        ));

        let stale_match_probe_receiver =
            fixture_output("get_unchecked_mut_match_get_reassigned_receiver_not_guard")?;
        let stale_match_probe_receiver_card = single_card(
            "get_unchecked_mut_match_get_reassigned_receiver_not_guard",
            &stale_match_probe_receiver,
        )?;
        assert_eq!(
            stale_match_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_match_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_match_probe_receiver_card,
            "bounds"
        ));

        let stale_match_probe_receiver_path =
            fixture_output("get_unchecked_mut_match_get_reassigned_receiver_path_not_guard")?;
        let stale_match_probe_receiver_path_card = single_card(
            "get_unchecked_mut_match_get_reassigned_receiver_path_not_guard",
            &stale_match_probe_receiver_path,
        )?;
        assert_eq!(
            stale_match_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            stale_match_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            stale_match_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_match_probe_receiver_path =
            fixture_output("get_unchecked_mut_match_get_shadowed_receiver_path_not_guard")?;
        let shadowed_match_probe_receiver_path_card = single_card(
            "get_unchecked_mut_match_get_shadowed_receiver_path_not_guard",
            &shadowed_match_probe_receiver_path,
        )?;
        assert_eq!(
            shadowed_match_probe_receiver_path_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_match_probe_receiver_path_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_match_probe_receiver_path_card,
            "bounds"
        ));

        let shadowed_match_probe_receiver =
            fixture_output("get_unchecked_mut_match_get_shadowed_receiver_not_guard")?;
        let shadowed_match_probe_receiver_card = single_card(
            "get_unchecked_mut_match_get_shadowed_receiver_not_guard",
            &shadowed_match_probe_receiver,
        )?;
        assert_eq!(
            shadowed_match_probe_receiver_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(
            shadowed_match_probe_receiver_card.class,
            ReviewClass::GuardMissing
        );
        assert!(!obligation_discharge_present(
            shadowed_match_probe_receiver_card,
            "bounds"
        ));

        let other_slice_probe =
            fixture_output("get_unchecked_mut_get_probe_other_slice_not_guard")?;
        let other_slice_probe_card = single_card(
            "get_unchecked_mut_get_probe_other_slice_not_guard",
            &other_slice_probe,
        )?;
        assert_eq!(
            other_slice_probe_card.operation.family,
            OperationFamily::GetUnchecked
        );
        assert_eq!(other_slice_probe_card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(
            other_slice_probe_card,
            "bounds"
        ));
        Ok(())
    }

    #[test]
    fn pin_new_unchecked_uses_pin_operation_family() -> Result<(), String> {
        let output = fixture_output("pin_new_unchecked")?;
        let card = single_card("pin_new_unchecked", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PinUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::PinInvariant));
        assert!(!obligation_discharge_present(card, "pin"));
        assert!(
            card.next_action.verify_commands.is_empty(),
            "Pin invariant review should not invent a generic witness command"
        );
        assert!(card.id.0.contains("new-unchecked"));
        Ok(())
    }

    #[test]
    fn adjacent_unchanged_unsafe_fn_is_not_reported_by_neighboring_change() -> Result<(), String> {
        let output = fixture_output("adjacent_unchanged_unsafe_fn_no_card")?;

        assert!(
            output.cards.is_empty(),
            "neighboring safe-code changes should not report an unchanged unsafe declaration"
        );
        Ok(())
    }

    #[test]
    fn slice_from_raw_parts_mut_uses_slice_operation_family() -> Result<(), String> {
        let output = fixture_output("slice_from_raw_parts_mut")?;
        let card = single_card("slice_from_raw_parts_mut", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::SliceFromRawParts);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.id.0.contains("from-raw-parts-mut"));
        Ok(())
    }

    #[test]
    fn slice_from_raw_parts_mut_maybeuninit_evidence_requires_slice_context() -> Result<(), String>
    {
        let output = fixture_output("slice_from_raw_parts_mut_other_maybeuninit_not_guard")?;
        let card = single_card(
            "slice_from_raw_parts_mut_other_maybeuninit_not_guard",
            &output,
        )?;

        assert_eq!(card.operation.family, OperationFamily::SliceFromRawParts);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "initialized"));
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_after_descriptor_capture_emits_advisory_card() -> Result<(), String> {
        let output = temp_source_output(
            "unsafe-review-js-buffer-reentry",
            r#"
pub struct JSValue;
pub struct GlobalObject;
pub struct Options;
pub struct StringOrBuffer;

impl StringOrBuffer {
    pub fn from_js(_global: &mut GlobalObject, _value: JSValue) -> Result<Self, ()> {
        Ok(Self)
    }

    pub fn byte_slice(&self) -> &[u8] {
        &[]
    }
}

impl Options {
    pub fn get(&self, _global: &mut GlobalObject, _name: &str) -> Result<i32, ()> {
        Ok(1)
    }
}

pub fn zstd_sync(
    global: &mut GlobalObject,
    arg0: JSValue,
    options: Options,
) -> Result<usize, ()> {
    let input = StringOrBuffer::from_js(global, arg0)?;
    let level = options.get(global, "level")?;
    native_compress(&input, level)
}

fn native_compress(input: &StringOrBuffer, level: i32) -> Result<usize, ()> {
    let bytes = input.byte_slice();
    Ok(bytes.len() + level as usize)
}
"#,
        )?;
        let card = single_card("js_buffer_reentry", &output)?;

        assert_eq!(card.operation.family, OperationFamily::UnsafeFnCall);
        assert_eq!(card.class, ReviewClass::ContractMissing);
        assert_eq!(card.site.owner.as_deref(), Some("zstd_sync"));
        assert!(
            card.operation
                .expression
                .contains("JS-backed buffer descriptor")
        );
        assert!(
            card.operation
                .expression
                .contains("StringOrBuffer::from_js")
        );
        assert!(card.operation.expression.contains("options.get"));
        assert!(card.operation.expression.contains("native_compress"));
        assert!(
            card.next_action
                .summary
                .contains("parse options before capture")
        );
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == WitnessKind::HumanDeepReview)
        );
        Ok(())
    }

    #[test]
    fn js_buffer_reentry_heuristic_respects_capture_before_reentry_order() -> Result<(), String> {
        let output = temp_source_output(
            "unsafe-review-js-buffer-reentry-negative",
            r#"
pub struct JSValue;
pub struct GlobalObject;
pub struct Options;
pub struct StringOrBuffer;

impl StringOrBuffer {
    pub fn from_js(_global: &mut GlobalObject, _value: JSValue) -> Result<Self, ()> {
        Ok(Self)
    }

    pub fn byte_slice(&self) -> &[u8] {
        &[]
    }
}

impl Options {
    pub fn get(&self, _global: &mut GlobalObject, _name: &str) -> Result<i32, ()> {
        Ok(1)
    }
}

pub fn zstd_sync(
    global: &mut GlobalObject,
    arg0: JSValue,
    options: Options,
) -> Result<usize, ()> {
    let level = options.get(global, "level")?;
    let input = StringOrBuffer::from_js(global, arg0)?;
    let bytes = input.byte_slice();
    Ok(bytes.len() + level as usize)
}
"#,
        )?;

        assert!(
            output.cards.is_empty(),
            "parsing options before descriptor capture should not trigger the reentry heuristic"
        );
        Ok(())
    }

    #[test]
    fn vec_from_raw_parts_uses_vec_operation_family() -> Result<(), String> {
        let output = fixture_output("vec_from_raw_parts")?;
        let card = single_card("vec_from_raw_parts", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::VecFromRawParts);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::DropOrDeallocation));
        assert!(card.id.0.contains("from-raw-parts"));
        Ok(())
    }

    #[test]
    fn vec_from_raw_parts_manuallydrop_origin_marks_same_origin_evidence() -> Result<(), String> {
        let output = fixture_output("vec_from_raw_parts_manuallydrop_origin")?;
        let card = single_card("vec_from_raw_parts_manuallydrop_origin", &output)?;

        assert_eq!(card.operation.family, OperationFamily::VecFromRawParts);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(obligation_discharge_present(card, "pointer-live"));
        assert!(obligation_discharge_present(card, "ownership"));
        assert!(!obligation_discharge_present(card, "alignment"));
        assert!(obligation_discharge_present(card, "capacity"));
        assert!(obligation_discharge_present(card, "initialized"));
        Ok(())
    }

    #[test]
    fn vec_from_raw_parts_capacity_evidence_requires_enforced_len_cap_guard() -> Result<(), String>
    {
        for fixture in [
            "vec_from_raw_parts_capacity_guard",
            "vec_from_raw_parts_capacity_assert_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::VecFromRawParts);
            assert!(obligation_discharge_present(card, "capacity"));
        }

        for fixture in [
            "vec_from_raw_parts_capacity_observed_not_guard",
            "vec_from_raw_parts_capacity_value_observed_not_guard",
            "vec_from_raw_parts_capacity_closed_branch_not_guard",
            "vec_from_raw_parts_capacity_reassigned_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.operation.family, OperationFamily::VecFromRawParts);
            assert!(!obligation_discharge_present(card, "capacity"));
        }
        Ok(())
    }

    #[test]
    fn box_from_raw_uses_ownership_operation_family() -> Result<(), String> {
        let output = fixture_output("box_from_raw")?;
        let card = single_card("box_from_raw", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::BoxFromRaw);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::DropOrDeallocation));
        assert!(card.id.0.contains("from-raw"));
        assert!(
            card.obligation_evidence
                .iter()
                .any(|evidence| evidence.obligation.key == "ownership"
                    && !evidence.discharge.present),
            "Box::from_raw ownership obligation should remain missing without allocator proof"
        );
        Ok(())
    }

    #[test]
    fn box_origin_evidence_rejects_reassigned_pointers() -> Result<(), String> {
        for (fixture, family) in [
            (
                "box_from_raw_reassigned_origin_not_guard",
                OperationFamily::BoxFromRaw,
            ),
            (
                "drop_in_place_reassigned_origin_not_guard",
                OperationFamily::DropInPlace,
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, family);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(card.hazards.contains(&HazardKind::DropOrDeallocation));
            assert!(!obligation_discharge_present(card, "ownership"));
        }
        Ok(())
    }

    #[test]
    fn static_mut_global_state_routes_to_concurrency_review() -> Result<(), String> {
        let output = fixture_output("static_mut_global_state")?;
        let card = single_card("static_mut_global_state", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::StaticMut);
        assert_eq!(card.operation.family, OperationFamily::StaticMut);
        assert_eq!(card.class, ReviewClass::RequiresLoom);
        assert!(card.hazards.contains(&HazardKind::StaticMutGlobalState));
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == WitnessKind::Loom)
        );
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == WitnessKind::Shuttle)
        );
        assert!(
            card.next_action.summary.contains("Loom/Shuttle"),
            "static mut global state should route reviewers to concurrency witnesses"
        );
        Ok(())
    }

    #[test]
    fn copy_nonoverlapping_uses_copy_operation_family() -> Result<(), String> {
        let output = fixture_output("copy_nonoverlapping")?;
        let card = single_card("copy_nonoverlapping", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::CopyNonOverlapping);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::AliasingOrProvenance));
        assert!(card.id.0.contains("copy-nonoverlapping"));
        Ok(())
    }

    #[test]
    fn ptr_copy_uses_overlapping_copy_operation_family() -> Result<(), String> {
        let output = fixture_output("ptr_copy_overlapping")?;
        let card = single_card("ptr_copy_overlapping", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PtrCopy);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::PointerValidity));
        assert!(!card.hazards.contains(&HazardKind::AliasingOrProvenance));
        assert!(
            card.obligations
                .iter()
                .all(|obligation| obligation.key != "non-overlap"),
            "ptr::copy permits overlap and should not inherit the copy_nonoverlapping obligation"
        );
        assert!(card.id.0.contains("ptr-copy"));
        Ok(())
    }

    #[test]
    fn copy_range_evidence_rejects_unrelated_length_guards() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_other_len_not_guard",
            "ptr_copy_other_len_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should not accept an unrelated length assertion as copy range evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_rejects_one_sided_slice_length_guards() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_slice_range_src_only_not_guard",
            "copy_nonoverlapping_slice_range_dst_only_not_guard",
            "ptr_copy_slice_range_src_only_not_guard",
            "ptr_copy_slice_range_dst_only_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should require both source and destination slice range evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_rejects_closed_slice_length_branches() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_slice_range_closed_branch_not_guard",
            "ptr_copy_slice_range_closed_branch_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should not accept closed branch observations as active range evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_rejects_or_slice_length_branches() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_slice_range_or_branch_not_guard",
            "ptr_copy_slice_range_or_branch_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should not accept disjunctive range branches as full range evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_rejects_comment_only_early_returns() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_slice_range_disjunctive_early_return_block_comment_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_block_comment_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should not accept commented return text as copy range evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_accepts_slice_length_guards() -> Result<(), String> {
        let copy_nonoverlapping = fixture_output("copy_nonoverlapping_slice_range_guard")?;
        let copy_nonoverlapping_card = single_card(
            "copy_nonoverlapping_slice_range_guard",
            &copy_nonoverlapping,
        )?;
        assert!(obligation_discharge_present(
            copy_nonoverlapping_card,
            "valid-range"
        ));
        assert!(
            !obligation_discharge_present(copy_nonoverlapping_card, "non-overlap"),
            "slice range guards should not prove non-overlap"
        );

        let ptr_copy = fixture_output("ptr_copy_slice_range_guard")?;
        let ptr_copy_card = single_card("ptr_copy_slice_range_guard", &ptr_copy)?;
        assert!(obligation_discharge_present(ptr_copy_card, "valid-range"));
        assert!(
            !obligation_discharge_present(ptr_copy_card, "initialized"),
            "range guards should not prove initialized memory"
        );
        Ok(())
    }

    #[test]
    fn copy_range_evidence_accepts_conjunctive_slice_length_guards() -> Result<(), String> {
        for (fixture, missing_key) in [
            (
                "copy_nonoverlapping_slice_range_conjunctive_assert_guard",
                "non-overlap",
            ),
            (
                "copy_nonoverlapping_slice_range_conjunctive_open_branch_guard",
                "non-overlap",
            ),
            (
                "ptr_copy_slice_range_conjunctive_assert_guard",
                "initialized",
            ),
            (
                "ptr_copy_slice_range_conjunctive_open_branch_guard",
                "initialized",
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;
            assert!(
                obligation_discharge_present(card, "valid-range"),
                "{fixture} should accept conjunctive source/destination slice range evidence"
            );
            assert!(
                !obligation_discharge_present(card, missing_key),
                "{fixture} should not prove {missing_key}"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_accepts_slice_length_early_returns() -> Result<(), String> {
        let copy_nonoverlapping =
            fixture_output("copy_nonoverlapping_slice_range_early_return_guard")?;
        let copy_nonoverlapping_card = single_card(
            "copy_nonoverlapping_slice_range_early_return_guard",
            &copy_nonoverlapping,
        )?;
        assert!(obligation_discharge_present(
            copy_nonoverlapping_card,
            "valid-range"
        ));
        assert!(
            !obligation_discharge_present(copy_nonoverlapping_card, "non-overlap"),
            "early-return range guards should not prove non-overlap"
        );

        let ptr_copy = fixture_output("ptr_copy_slice_range_early_return_guard")?;
        let ptr_copy_card = single_card("ptr_copy_slice_range_early_return_guard", &ptr_copy)?;
        assert!(obligation_discharge_present(ptr_copy_card, "valid-range"));
        assert!(
            !obligation_discharge_present(ptr_copy_card, "initialized"),
            "early-return range guards should not prove initialized memory"
        );
        Ok(())
    }

    #[test]
    fn copy_range_evidence_accepts_disjunctive_slice_length_early_returns() -> Result<(), String> {
        for (fixture, missing_key) in [
            (
                "copy_nonoverlapping_slice_range_disjunctive_early_return_guard",
                "non-overlap",
            ),
            (
                "ptr_copy_slice_range_disjunctive_early_return_guard",
                "initialized",
            ),
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;
            assert!(
                obligation_discharge_present(card, "valid-range"),
                "{fixture} should accept disjunctive invalid-range early returns as range evidence"
            );
            assert!(
                !obligation_discharge_present(card, missing_key),
                "{fixture} should not prove {missing_key}"
            );
        }
        Ok(())
    }

    #[test]
    fn copy_range_evidence_accepts_open_slice_length_branches() -> Result<(), String> {
        let copy_nonoverlapping =
            fixture_output("copy_nonoverlapping_slice_range_open_branch_guard")?;
        let copy_nonoverlapping_card = single_card(
            "copy_nonoverlapping_slice_range_open_branch_guard",
            &copy_nonoverlapping,
        )?;
        assert!(obligation_discharge_present(
            copy_nonoverlapping_card,
            "valid-range"
        ));
        assert!(
            !obligation_discharge_present(copy_nonoverlapping_card, "non-overlap"),
            "open-branch range guards should not prove non-overlap"
        );

        let ptr_copy = fixture_output("ptr_copy_slice_range_open_branch_guard")?;
        let ptr_copy_card = single_card("ptr_copy_slice_range_open_branch_guard", &ptr_copy)?;
        assert!(obligation_discharge_present(ptr_copy_card, "valid-range"));
        assert!(
            !obligation_discharge_present(ptr_copy_card, "initialized"),
            "open-branch range guards should not prove initialized memory"
        );
        Ok(())
    }

    #[test]
    fn copy_range_evidence_rejects_stale_slice_length_guards() -> Result<(), String> {
        for fixture in [
            "copy_nonoverlapping_slice_range_open_branch_reassigned_count_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_compound_reassigned_count_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_shadowed_count_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_reassigned_src_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_reassigned_src_path_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_shadowed_src_path_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_shadowed_src_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_reassigned_dst_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_reassigned_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_shadowed_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_open_branch_shadowed_dst_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_count_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_compound_reassigned_count_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_count_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_src_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_src_path_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_src_path_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_src_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_dst_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_reassigned_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_disjunctive_early_return_shadowed_dst_not_guard",
            "copy_nonoverlapping_slice_range_reassigned_count_not_guard",
            "copy_nonoverlapping_slice_range_shadowed_count_not_guard",
            "copy_nonoverlapping_slice_range_reassigned_src_not_guard",
            "copy_nonoverlapping_slice_range_reassigned_src_path_not_guard",
            "copy_nonoverlapping_slice_range_shadowed_src_path_not_guard",
            "copy_nonoverlapping_slice_range_shadowed_src_not_guard",
            "copy_nonoverlapping_slice_range_reassigned_dst_not_guard",
            "copy_nonoverlapping_slice_range_reassigned_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_shadowed_dst_path_not_guard",
            "copy_nonoverlapping_slice_range_shadowed_dst_not_guard",
            "ptr_copy_slice_range_open_branch_reassigned_count_not_guard",
            "ptr_copy_slice_range_open_branch_reassigned_dst_not_guard",
            "ptr_copy_slice_range_open_branch_reassigned_src_path_not_guard",
            "ptr_copy_slice_range_open_branch_reassigned_dst_path_not_guard",
            "ptr_copy_slice_range_open_branch_shadowed_src_path_not_guard",
            "ptr_copy_slice_range_open_branch_shadowed_dst_path_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_reassigned_count_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_compound_reassigned_count_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_shadowed_count_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_reassigned_src_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_reassigned_src_path_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_shadowed_src_path_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_shadowed_src_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_reassigned_dst_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_reassigned_dst_path_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_shadowed_dst_path_not_guard",
            "ptr_copy_slice_range_disjunctive_early_return_shadowed_dst_not_guard",
            "ptr_copy_slice_range_reassigned_count_not_guard",
            "ptr_copy_slice_range_shadowed_count_not_guard",
            "ptr_copy_slice_range_open_branch_reassigned_src_not_guard",
            "ptr_copy_slice_range_open_branch_compound_reassigned_count_not_guard",
            "ptr_copy_slice_range_open_branch_shadowed_count_not_guard",
            "ptr_copy_slice_range_open_branch_shadowed_src_not_guard",
            "ptr_copy_slice_range_open_branch_shadowed_dst_not_guard",
            "ptr_copy_slice_range_reassigned_src_not_guard",
            "ptr_copy_slice_range_reassigned_src_path_not_guard",
            "ptr_copy_slice_range_shadowed_src_path_not_guard",
            "ptr_copy_slice_range_shadowed_src_not_guard",
            "ptr_copy_slice_range_reassigned_dst_not_guard",
            "ptr_copy_slice_range_reassigned_dst_path_not_guard",
            "ptr_copy_slice_range_shadowed_dst_path_not_guard",
            "ptr_copy_slice_range_shadowed_dst_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert!(
                !obligation_discharge_present(card, "valid-range"),
                "{fixture} should not accept stale slice length evidence after reassignment"
            );
        }
        Ok(())
    }

    #[test]
    fn ptr_replace_uses_replacement_operation_family() -> Result<(), String> {
        let output = fixture_output("ptr_replace_value")?;
        let card = single_card("ptr_replace_value", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PtrReplace);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::PointerValidity));
        assert!(card.hazards.contains(&HazardKind::DropOrDeallocation));
        assert!(
            card.obligations
                .iter()
                .any(|obligation| obligation.key == "ownership"),
            "ptr::replace should preserve drop/ownership review evidence"
        );
        assert!(card.id.0.contains("ptr-replace"));
        Ok(())
    }

    #[test]
    fn maybeuninit_assume_init_accepts_same_slot_initialization_evidence() -> Result<(), String> {
        for fixture in [
            "maybeuninit_assume_init_write_guard",
            "maybeuninit_assume_init_read_write_guard",
            "maybeuninit_assume_init_ref_write_guard",
            "maybeuninit_assume_init_mut_write_guard",
            "maybeuninit_assume_init_drop_write_guard",
            "maybeuninit_assume_init_open_branch_write_guard",
            "maybeuninit_assume_init_read_open_branch_write_guard",
            "maybeuninit_assume_init_ref_open_branch_write_guard",
            "maybeuninit_assume_init_mut_open_branch_write_guard",
            "maybeuninit_assume_init_drop_open_branch_write_guard",
            "maybeuninit_assume_init_open_branch_new_guard",
            "maybeuninit_assume_init_read_open_branch_new_guard",
            "maybeuninit_assume_init_ref_open_branch_new_guard",
            "maybeuninit_assume_init_mut_open_branch_new_guard",
            "maybeuninit_assume_init_drop_open_branch_new_guard",
            "maybeuninit_assume_init_new_guard",
            "maybeuninit_assume_init_read_new_guard",
            "maybeuninit_assume_init_ref_new_guard",
            "maybeuninit_assume_init_mut_method_new_guard",
            "maybeuninit_assume_init_mut_new_guard",
            "maybeuninit_assume_init_drop_new_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(
                card.operation.family,
                OperationFamily::MaybeUninitAssumeInit
            );
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().all(|missing| missing.kind != "guard"),
                "{fixture} should resolve the local initialized-memory guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn maybeuninit_assume_init_rejects_non_dominating_or_stale_slot_evidence() -> Result<(), String>
    {
        for fixture in [
            "maybeuninit_assume_init_comment_not_guard",
            "maybeuninit_assume_init_closed_branch_write_not_guard",
            "maybeuninit_assume_init_read_closed_branch_write_not_guard",
            "maybeuninit_assume_init_ref_closed_branch_write_not_guard",
            "maybeuninit_assume_init_mut_closed_branch_write_not_guard",
            "maybeuninit_assume_init_drop_closed_branch_write_not_guard",
            "maybeuninit_assume_init_closed_branch_new_not_guard",
            "maybeuninit_assume_init_read_closed_branch_new_not_guard",
            "maybeuninit_assume_init_ref_closed_branch_new_not_guard",
            "maybeuninit_assume_init_mut_closed_branch_new_not_guard",
            "maybeuninit_assume_init_drop_closed_branch_new_not_guard",
            "maybeuninit_assume_init_other_slot_write_not_guard",
            "maybeuninit_assume_init_stale_write_not_guard",
            "maybeuninit_assume_init_read_stale_write_not_guard",
            "maybeuninit_assume_init_ref_stale_write_not_guard",
            "maybeuninit_assume_init_mut_stale_write_not_guard",
            "maybeuninit_assume_init_drop_stale_write_not_guard",
            "maybeuninit_assume_init_read_other_slot_write_not_guard",
            "maybeuninit_assume_init_ref_other_slot_write_not_guard",
            "maybeuninit_assume_init_mut_other_slot_write_not_guard",
            "maybeuninit_assume_init_drop_other_slot_write_not_guard",
            "maybeuninit_assume_init_stale_field_write_not_guard",
            "maybeuninit_assume_init_stale_new_not_guard",
            "maybeuninit_assume_init_read_stale_new_not_guard",
            "maybeuninit_assume_init_ref_stale_new_not_guard",
            "maybeuninit_assume_init_mut_stale_new_not_guard",
            "maybeuninit_assume_init_drop_stale_new_not_guard",
            "maybeuninit_assume_init_shadowed_slot_not_guard",
            "maybeuninit_assume_init_read_shadowed_slot_not_guard",
            "maybeuninit_assume_init_ref_shadowed_slot_not_guard",
            "maybeuninit_assume_init_mut_shadowed_slot_not_guard",
            "maybeuninit_assume_init_drop_shadowed_slot_not_guard",
            "maybeuninit_assume_init_mutslot_new_not_guard",
            "maybeuninit_assume_init_read_mutslot_new_not_guard",
            "maybeuninit_assume_init_ref_mutslot_new_not_guard",
            "maybeuninit_assume_init_mut_mutslot_new_not_guard",
            "maybeuninit_assume_init_drop_mutslot_new_not_guard",
            "maybeuninit_assume_init_partial_field_not_guard",
            "maybeuninit_assume_init_partial_array_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(
                card.operation.family,
                OperationFamily::MaybeUninitAssumeInit
            );
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must not resolve the initialized-memory guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn maybeuninit_assume_init_read_uses_assume_init_operation_family() -> Result<(), String> {
        let output = fixture_output("maybeuninit_assume_init_read")?;
        let card = single_card("maybeuninit_assume_init_read", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(
            card.operation.family,
            OperationFamily::MaybeUninitAssumeInit
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InitializedMemory));
        assert!(
            card.obligation_evidence.iter().any(|evidence| {
                evidence.obligation.key == "initialized" && !evidence.discharge.present
            }),
            "assume_init_read should keep initialized-memory evidence missing without proof"
        );
        assert!(card.id.0.contains("assume-init-read"));
        Ok(())
    }

    #[test]
    fn maybeuninit_assume_init_ref_uses_assume_init_operation_family() -> Result<(), String> {
        let output = fixture_output("maybeuninit_assume_init_ref")?;
        let card = single_card("maybeuninit_assume_init_ref", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(
            card.operation.family,
            OperationFamily::MaybeUninitAssumeInit
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InitializedMemory));
        assert!(
            card.obligation_evidence.iter().any(|evidence| {
                evidence.obligation.key == "initialized" && !evidence.discharge.present
            }),
            "assume_init_ref should keep initialized-memory evidence missing without proof"
        );
        assert!(card.id.0.contains("assume-init-ref"));
        Ok(())
    }

    #[test]
    fn maybeuninit_assume_init_mut_uses_assume_init_operation_family() -> Result<(), String> {
        let output = fixture_output("maybeuninit_assume_init_mut")?;
        let card = single_card("maybeuninit_assume_init_mut", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(
            card.operation.family,
            OperationFamily::MaybeUninitAssumeInit
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InitializedMemory));
        assert!(
            card.obligation_evidence.iter().any(|evidence| {
                evidence.obligation.key == "initialized" && !evidence.discharge.present
            }),
            "assume_init_mut should keep initialized-memory evidence missing without proof"
        );
        assert!(card.id.0.contains("assume-init-mut"));
        Ok(())
    }

    #[test]
    fn maybeuninit_assume_init_drop_uses_assume_init_operation_family() -> Result<(), String> {
        let output = fixture_output("maybeuninit_assume_init_drop")?;
        let card = single_card("maybeuninit_assume_init_drop", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(
            card.operation.family,
            OperationFamily::MaybeUninitAssumeInit
        );
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InitializedMemory));
        assert!(
            card.obligation_evidence.iter().any(|evidence| {
                evidence.obligation.key == "initialized" && !evidence.discharge.present
            }),
            "assume_init_drop should keep initialized-memory evidence missing without proof"
        );
        assert!(card.id.0.contains("assume-init-drop"));
        Ok(())
    }

    #[test]
    fn vec_set_len_capacity_observation_is_not_capacity_guard() -> Result<(), String> {
        for fixture in [
            "vec_set_len_capacity_observed_not_guard",
            "vec_set_len_unrelated_capacity_comparison_not_guard",
            "vec_set_len_cap_argument_not_guard",
            "vec_set_len_stale_capacity_binding_not_guard",
            "vec_set_len_stale_start_bound_shrink_not_guard",
            "vec_set_len_stale_last_index_shrink_not_guard",
            "vec_set_len_reassigned_receiver_not_guard",
            "vec_set_len_reassigned_new_len_not_guard",
            "vec_set_len_compound_reassigned_new_len_not_guard",
            "vec_set_len_shadowed_new_len_not_guard",
            "vec_set_len_reserve_reassigned_additional_not_guard",
            "vec_set_len_try_reserve_reassigned_additional_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::VecSetLen);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "capacity"));
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "observing capacity without bounding new_len must keep the guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn vec_set_len_self_new_const_cap_keeps_capacity_missing() -> Result<(), String> {
        let output = fixture_output("vec_set_len_self_new_const_cap_not_guard")?;
        let card = single_card("vec_set_len_self_new_const_cap_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::VecSetLen);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "capacity"));
        assert!(obligation_discharge_present(card, "initialized"));
        assert!(
            card.missing.iter().any(|missing| missing.kind == "guard"),
            "opaque Self::new capacity evidence must keep the guard prompt"
        );
        Ok(())
    }

    #[test]
    fn vec_set_len_accepts_pre_call_initialized_range_evidence() -> Result<(), String> {
        for fixture in [
            "vec_set_len_initialized_loop",
            "vec_set_len_slice_binding_initialized_loop",
            "vec_set_len_call_result_init",
            "vec_set_len_shrink",
            "vec_set_len_last_index_shrink",
            "vec_set_len_start_bound_shrink",
            "vec_set_len_zero_clear",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::VecSetLen);
            assert!(
                matches!(
                    card.class,
                    ReviewClass::GuardedUnwitnessed | ReviewClass::UnsafeUnreached
                ),
                "{fixture} should have initialized-range evidence without an open guard prompt"
            );
            assert!(obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().all(|missing| missing.kind != "guard"),
                "{fixture} should resolve the local initialized-range guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn vec_set_len_capacity_only_evidence_keeps_initialized_range_gap() -> Result<(), String> {
        for fixture in [
            "vec_set_len_capacity_binding",
            "vec_set_len_with_capacity",
            "vec_set_len_reserve_capacity",
            "vec_set_len_try_reserve_capacity",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::VecSetLen);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "capacity"));
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} should keep the initialized-range guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn vec_set_len_post_initialization_is_not_guard_evidence() -> Result<(), String> {
        for fixture in [
            "vec_set_len_post_init_not_guard",
            "vec_set_len_unrelated_initialization_not_guard",
            "vec_set_len_other_slice_binding_not_guard",
            "vec_set_len_partial_slice_binding_not_guard",
            "vec_set_len_stale_slice_binding_not_guard",
            "vec_set_len_single_index_init_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::VecSetLen);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "capacity"));
            assert!(!obligation_discharge_present(card, "initialized"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must keep the initialization guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn str_from_utf8_unchecked_uses_utf8_operation_family() -> Result<(), String> {
        for fixture in [
            "str_from_utf8_unchecked",
            "str_from_utf8_unchecked_comment_not_guard",
            "str_from_utf8_unchecked_post_validation_not_guard",
            "str_from_utf8_unchecked_other_buffer_not_guard",
            "str_from_utf8_unchecked_prefix_validation_not_guard",
            "str_from_utf8_unchecked_suffix_validation_not_guard",
            "str_from_utf8_unchecked_if_let_err_return_comment_not_guard",
            "str_from_utf8_unchecked_if_let_err_return_string_not_guard",
            "str_from_utf8_unchecked_is_err_return_comment_not_guard",
            "str_from_utf8_unchecked_is_err_return_string_not_guard",
            "str_from_utf8_unchecked_is_ok_observed_not_guard",
            "str_from_utf8_unchecked_guard_then_reassigned_not_guard",
            "str_from_utf8_unchecked_guard_then_mutated_not_guard",
            "str_from_utf8_unchecked_guard_then_shadowed_not_guard",
            "str_from_utf8_unchecked_is_err_return_reassigned_not_guard",
            "str_from_utf8_unchecked_is_err_return_shadowed_not_guard",
            "str_from_utf8_unchecked_if_let_ok_comment_not_guard",
            "str_from_utf8_unchecked_if_let_ok_string_not_guard",
            "str_from_utf8_unchecked_question_mark_comment_not_guard",
            "str_from_utf8_unchecked_question_mark_string_not_guard",
            "str_from_utf8_unchecked_match_return_comment_not_guard",
            "str_from_utf8_unchecked_match_return_string_not_guard",
            "str_from_utf8_unchecked_if_let_ok_reassigned_not_guard",
            "str_from_utf8_unchecked_if_let_ok_shadowed_not_guard",
            "str_from_utf8_unchecked_if_let_err_shadowed_not_guard",
            "str_from_utf8_unchecked_match_err_reassigned_not_guard",
            "str_from_utf8_unchecked_match_err_shadowed_not_guard",
            "str_from_utf8_unchecked_let_else_ok_shadowed_not_guard",
            "str_from_utf8_unchecked_match_ok_shadowed_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::StrFromUtf8Unchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(card.hazards.contains(&HazardKind::InvalidValue));
            assert!(card.id.0.contains("from-utf8-unchecked"));
            assert!(
                card.obligation_evidence.iter().any(
                    |evidence| evidence.obligation.key == "utf8" && !evidence.discharge.present
                ),
                "UTF-8 validation obligation should remain missing without a visible guard"
            );
        }
        Ok(())
    }

    #[test]
    fn str_from_utf8_unchecked_validation_guards_are_discharged() -> Result<(), String> {
        for fixture in [
            "str_from_utf8_unchecked_is_ok_guard",
            "str_from_utf8_unchecked_is_err_return_guard",
            "str_from_utf8_unchecked_question_mark_guard",
            "str_from_utf8_unchecked_match_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::StrFromUtf8Unchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(
                card.obligation_evidence.iter().any(
                    |evidence| evidence.obligation.key == "utf8" && evidence.discharge.present
                ),
                "{fixture} should discharge same-buffer UTF-8 validation evidence"
            );
        }
        Ok(())
    }

    #[test]
    fn transmute_bool_value_observation_is_not_guard_evidence() -> Result<(), String> {
        for fixture in [
            "transmute_layout_size_guard",
            "transmute_bool_comment_not_guard",
            "transmute_bool_other_value_not_guard",
            "transmute_bool_prior_guarded_call_not_guard",
            "transmute_bool_value_observed_not_guard",
            "transmute_bool_closed_if_observed_not_guard",
            "transmute_bool_invalid_return_comment_not_guard",
            "transmute_bool_guard_then_reassigned_not_guard",
            "transmute_bool_guard_then_compound_reassigned_not_guard",
            "transmute_bool_guard_then_shadowed_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::Transmute);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "layout"));
            assert!(!obligation_discharge_present(card, "valid-value"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must not resolve this card's guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn transmute_layout_branch_evidence_requires_top_level_conjunct() -> Result<(), String> {
        for fixture in [
            "transmute_layout_conjunct_branch_guard",
            "transmute_copy_layout_conjunct_branch_guard",
        ] {
            let guarded = fixture_output(fixture)?;
            let guarded_card = single_card(fixture, &guarded)?;

            assert_eq!(guarded_card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(guarded_card.operation.family, OperationFamily::Transmute);
            assert_eq!(guarded_card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(guarded_card, "layout"));
            assert!(!obligation_discharge_present(guarded_card, "valid-value"));
        }

        for fixture in [
            "transmute_layout_disjunct_branch_not_guard",
            "transmute_copy_layout_disjunct_branch_not_guard",
        ] {
            let disjunct = fixture_output(fixture)?;
            let disjunct_card = single_card(fixture, &disjunct)?;

            assert_eq!(disjunct_card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(disjunct_card.operation.family, OperationFamily::Transmute);
            assert_eq!(disjunct_card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(disjunct_card, "layout"));
            assert!(!obligation_discharge_present(disjunct_card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn transmute_layout_mismatch_return_requires_top_level_disjunct() -> Result<(), String> {
        for fixture in [
            "transmute_layout_mismatch_return_guard",
            "transmute_copy_layout_mismatch_return_guard",
        ] {
            let guarded = fixture_output(fixture)?;
            let guarded_card = single_card(fixture, &guarded)?;

            assert_eq!(guarded_card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(guarded_card.operation.family, OperationFamily::Transmute);
            assert_eq!(guarded_card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(guarded_card, "layout"));
            assert!(!obligation_discharge_present(guarded_card, "valid-value"));
        }

        for fixture in [
            "transmute_layout_conjunct_return_not_guard",
            "transmute_layout_mismatch_return_comment_not_guard",
            "transmute_copy_layout_conjunct_return_not_guard",
            "transmute_copy_layout_mismatch_return_comment_not_guard",
        ] {
            let conjunct = fixture_output(fixture)?;
            let conjunct_card = single_card(fixture, &conjunct)?;

            assert_eq!(conjunct_card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(conjunct_card.operation.family, OperationFamily::Transmute);
            assert_eq!(conjunct_card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(conjunct_card, "layout"));
            assert!(!obligation_discharge_present(conjunct_card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn transmute_bool_value_domain_guards_are_discharged() -> Result<(), String> {
        for fixture in [
            "transmute_bool_valid_value_guard",
            "transmute_bool_invalid_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::Transmute);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(obligation_discharge_present(card, "layout"));
            assert!(obligation_discharge_present(card, "valid-value"));
            assert!(
                card.missing.iter().all(|missing| missing.kind != "guard"),
                "{fixture} should resolve the local valid-value guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn transmute_copy_bool_value_observation_is_not_guard_evidence() -> Result<(), String> {
        for fixture in [
            "transmute_copy_layout_size_guard",
            "transmute_copy_bool_comment_not_guard",
            "transmute_copy_bool_other_value_not_guard",
            "transmute_copy_bool_prior_guarded_call_not_guard",
            "transmute_copy_bool_disjunct_branch_not_guard",
            "transmute_copy_bool_conjunct_return_not_guard",
            "transmute_copy_bool_value_observed_not_guard",
            "transmute_copy_bool_closed_if_observed_not_guard",
            "transmute_copy_bool_invalid_return_comment_not_guard",
            "transmute_copy_bool_guard_then_reassigned_not_guard",
            "transmute_copy_bool_guard_then_compound_reassigned_not_guard",
            "transmute_copy_bool_guard_then_shadowed_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::Transmute);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(obligation_discharge_present(card, "layout"));
            assert!(!obligation_discharge_present(card, "valid-value"));
            assert!(
                card.missing.iter().any(|missing| missing.kind == "guard"),
                "{fixture} must not resolve this card's guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn transmute_copy_bool_value_domain_guards_are_discharged() -> Result<(), String> {
        for fixture in [
            "transmute_copy_bool_valid_value_guard",
            "transmute_copy_bool_conjunct_branch_guard",
            "transmute_copy_bool_invalid_return_guard",
            "transmute_copy_bool_disjunct_return_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::Transmute);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(obligation_discharge_present(card, "layout"));
            assert!(obligation_discharge_present(card, "valid-value"));
            assert!(
                card.missing.iter().all(|missing| missing.kind != "guard"),
                "{fixture} should resolve the local valid-value guard prompt"
            );
        }
        Ok(())
    }

    #[test]
    fn zeroed_uses_valid_zero_operation_family() -> Result<(), String> {
        let output = fixture_output("zeroed_invalid_value")?;
        let card = single_card("zeroed_invalid_value", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::Zeroed);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InvalidValue));
        assert!(card.id.0.contains("zeroed"));
        assert!(
            card.obligation_evidence.iter().any(|evidence| {
                evidence.obligation.key == "valid-zero" && !evidence.discharge.present
            }),
            "valid-zero obligation should remain missing without target-type evidence"
        );
        Ok(())
    }

    #[test]
    fn inline_asm_uses_inline_asm_operation_family() -> Result<(), String> {
        let output = fixture_output("inline_asm_human_review")?;
        let card = single_card("inline_asm_human_review", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::InlineAsm);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InlineAsm));
        assert!(card.hazards.contains(&HazardKind::TargetFeature));
        assert!(
            card.routes
                .iter()
                .any(|route| route.kind == WitnessKind::HumanDeepReview),
            "inline asm should route to human deep review"
        );
        assert!(
            card.next_action.verify_commands.is_empty(),
            "human-review-only inline asm route should not invent a witness command"
        );
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_num_ctrl_bytes_guard_is_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_num_ctrl_bytes_guard")?;
        let card = single_card("pointer_arithmetic_num_ctrl_bytes_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.discharge.present);
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_other_offset_guard_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_other_offset_not_guard")?;
        let card = single_card("pointer_arithmetic_other_offset_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_reassigned_offset_guard_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_reassigned_offset_not_guard")?;
        let card = single_card("pointer_arithmetic_reassigned_offset_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_shadowed_offset_guard_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_shadowed_offset_not_guard")?;
        let card = single_card("pointer_arithmetic_shadowed_offset_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_compound_offset_guard_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_compound_offset_not_guard")?;
        let card = single_card("pointer_arithmetic_compound_offset_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_stale_bound_guard_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_stale_bound_not_guard")?;
        let card = single_card("pointer_arithmetic_stale_bound_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_disjunctive_bounds_branch_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_disjunct_bounds_not_guard")?;
        let card = single_card("pointer_arithmetic_disjunct_bounds_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_closed_bounds_branch_is_not_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_closed_branch_not_guard")?;
        let card = single_card("pointer_arithmetic_closed_branch_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.discharge.present);
        assert!(!obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn pointer_arithmetic_slice_end_guard_is_discharged() -> Result<(), String> {
        let output = fixture_output("pointer_arithmetic_slice_end")?;
        let card = single_card("pointer_arithmetic_slice_end", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::PointerArithmetic);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.discharge.present);
        assert!(obligation_discharge_present(card, "bounds"));
        assert!(card.id.0.contains("add"));
        Ok(())
    }

    #[test]
    fn documented_target_feature_declaration_is_guarded() -> Result<(), String> {
        let output = fixture_output("target_feature_safety_docs")?;
        let card = single_card("target_feature_safety_docs", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::TargetFeature);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.contract.present);
        assert!(card.discharge.present);
        assert!(obligation_discharge_present(card, "target-feature"));
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_uses_concrete_operation_family() -> Result<(), String> {
        let output = fixture_output("unwrap_unchecked_result")?;
        let card = single_card("unwrap_unchecked_result", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InvalidValue));
        assert!(card.id.0.contains("unwrap-unchecked"));
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_infallible_result_evidence_is_discharged() -> Result<(), String> {
        let output = fixture_output("unwrap_unchecked_infallible_result")?;
        let card = single_card("unwrap_unchecked_infallible_result", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.discharge.present);
        assert!(obligation_discharge_present(card, "valid-value"));
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_infallible_result_evidence_requires_same_receiver() -> Result<(), String> {
        let output = fixture_output("unwrap_unchecked_other_infallible_not_guard")?;
        let card = single_card("unwrap_unchecked_other_infallible_not_guard", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!obligation_discharge_present(card, "valid-value"));
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_if_let_as_ref_evidence_is_discharged() -> Result<(), String> {
        for fixture in [
            "unwrap_unchecked_if_let_some_guard",
            "unwrap_unchecked_if_let_ok_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
            assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
            assert!(obligation_discharge_present(card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn unwrap_unchecked_state_evidence_rejects_wrong_receiver_or_post_check() -> Result<(), String>
    {
        for fixture in [
            "unwrap_unchecked_is_some_observed_not_guard",
            "unwrap_unchecked_is_ok_observed_not_guard",
            "unwrap_unchecked_other_if_let_not_guard",
            "unwrap_unchecked_other_if_let_ok_not_guard",
            "unwrap_unchecked_post_check_not_guard",
            "unwrap_unchecked_guard_then_reassigned_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnwrapUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "valid-value"));
        }
        Ok(())
    }

    #[test]
    fn unreachable_unchecked_uses_concrete_operation_family() -> Result<(), String> {
        let output = fixture_output("unreachable_unchecked_path")?;
        let card = single_card("unreachable_unchecked_path", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnreachableUnchecked);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.hazards.contains(&HazardKind::InvalidValue));
        assert!(card.id.0.contains("unreachable-unchecked"));
        Ok(())
    }

    #[test]
    fn unreachable_unchecked_infallible_path_evidence_is_discharged() -> Result<(), String> {
        let output = fixture_output("unreachable_unchecked_infallible_path")?;
        let card = single_card("unreachable_unchecked_infallible_path", &output)?;

        assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
        assert_eq!(card.operation.family, OperationFamily::UnreachableUnchecked);
        assert_eq!(card.class, ReviewClass::GuardedUnwitnessed);
        assert!(card.discharge.present);
        assert!(obligation_discharge_present(card, "unreachable"));
        Ok(())
    }

    #[test]
    fn unreachable_unchecked_infallible_path_evidence_requires_same_match() -> Result<(), String> {
        for fixture in [
            "unreachable_unchecked_other_infallible_not_guard",
            "unreachable_unchecked_post_infallible_not_guard",
            "unreachable_unchecked_closed_infallible_match_not_guard",
        ] {
            let output = fixture_output(fixture)?;
            let card = single_card(fixture, &output)?;

            assert_eq!(card.site.kind, UnsafeSiteKind::Operation);
            assert_eq!(card.operation.family, OperationFamily::UnreachableUnchecked);
            assert_eq!(card.class, ReviewClass::GuardMissing);
            assert!(!obligation_discharge_present(card, "unreachable"));
        }
        Ok(())
    }

    #[test]
    fn impl_trait_bound_owner_inference_uses_function_owner() -> Result<(), String> {
        let output = fixture_output("impl_trait_bound_owner_inference")?;
        let card = single_card("impl_trait_bound_owner_inference", &output)?;

        assert_eq!(card.operation.family, OperationFamily::UnsafeFnCall);
        assert_eq!(card.site.owner, Some("try_reserve".to_string()));
        assert!(card.id.0.contains("try-reserve"));
        assert!(
            card.reach.summary.contains("try_reserve"),
            "reach evidence should use the function owner, not the impl Trait bound"
        );
        Ok(())
    }

    #[test]
    fn long_unsafe_fn_owner_inference_uses_enclosing_function_owner() -> Result<(), String> {
        let output = fixture_output("long_unsafe_fn_owner_inference")?;
        let card = output
            .cards
            .iter()
            .find(|card| card.operation.family == OperationFamily::DropInPlace)
            .ok_or_else(|| {
                format!(
                    "long_unsafe_fn_owner_inference should emit a drop_in_place card: {:#?}",
                    output.cards
                )
            })?;

        assert_eq!(card.operation.family, OperationFamily::DropInPlace);
        assert_eq!(card.site.owner, Some("run".to_string()));
        assert!(
            card.id.0.contains("-run-operation-drop_in_place-"),
            "card identity should include the enclosing owner: {}",
            card.id.0
        );
        Ok(())
    }

    #[test]
    fn macro_rules_owner_inference_uses_macro_name() -> Result<(), String> {
        let output = fixture_output("macro_rules_owner_inference")?;
        let card = single_card("macro_rules_owner_inference", &output)?;

        assert_eq!(card.operation.family, OperationFamily::BoxFromRaw);
        assert_eq!(card.site.owner, Some("spawn_unchecked".to_string()));
        assert!(
            card.id
                .0
                .contains("-spawn-unchecked-operation-box_from_raw-"),
            "card identity should include the macro owner: {}",
            card.id.0
        );
        Ok(())
    }

    #[test]
    fn private_unsafe_helper_can_use_local_safety_comment() -> Result<(), String> {
        let output = fixture_output("private_unsafe_helper_safety_comment")?;
        let card = single_card("private_unsafe_helper_safety_comment", &output)?;

        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(!card.site.public_api_surface);
        assert!(card.contract.present);
        assert_eq!(card.reach.state, "owner_reached");
        assert!(
            !card
                .missing
                .iter()
                .any(|missing| missing.kind == "contract")
        );
        assert!(
            !card.next_action.summary.contains("`unknown`"),
            "unknown operation next action should not ask reviewers to discharge an unknown obligation"
        );
        assert!(card.next_action.summary.contains("unsafe site"));
        assert!(
            card.next_action
                .summary
                .contains("obligation-specific guard")
        );
        Ok(())
    }

    #[test]
    fn card_identity_is_stable_across_line_drift() -> Result<(), String> {
        let original = fixture_output("raw_pointer_alignment")?;
        let drifted = fixture_output("raw_pointer_alignment_line_drift")?;
        let original_card = single_card("raw_pointer_alignment", &original)?;
        let drifted_card = single_card("raw_pointer_alignment_line_drift", &drifted)?;

        assert_ne!(
            original_card.site.location.line, drifted_card.site.location.line,
            "fixture should prove line drift occurred"
        );
        assert_eq!(
            original_card.id, drifted_card.id,
            "card identity should not include the line number"
        );
        Ok(())
    }

    #[test]
    fn card_identity_includes_exact_policy_matching_components() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let card = single_card("raw_pointer_alignment", &output)?;

        assert_eq!(
            card.id.0,
            "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1"
        );
        Ok(())
    }

    #[test]
    fn card_identity_counts_duplicate_sites() -> Result<(), String> {
        let output = fixture_output("duplicate_raw_pointer_reads")?;
        if output.cards.len() != 2 {
            return Err(format!(
                "duplicate_raw_pointer_reads should emit two cards, got {}",
                output.cards.len()
            ));
        }
        let first = &output.cards[0];
        let second = &output.cards[1];

        assert_eq!(first.operation.family, OperationFamily::RawPointerRead);
        assert_eq!(second.operation.family, OperationFamily::RawPointerRead);
        assert_ne!(first.id, second.id);
        assert!(first.id.0.ends_with("-c1"));
        assert!(second.id.0.ends_with("-c2"));
        assert_eq!(
            identity_without_count(&first.id),
            identity_without_count(&second.id)
        );
        Ok(())
    }

    #[test]
    fn baseline_policy_marks_exact_card_identity_as_known() -> Result<(), String> {
        let root = copy_fixture_to_temp("raw_pointer_alignment", "unsafe-review-baseline-match")?;
        let card_id = single_card("raw_pointer_alignment", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_policy_ledger(
            &root,
            "unsafe-review-baseline.toml",
            &card_id,
            "review_after",
        )?;

        let output = fixture_output_at(&root)?;
        let card = single_card("raw_pointer_alignment baseline", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert_eq!(card.class, ReviewClass::BaselineKnown);
        assert_eq!(card.priority, Priority::Low);
        assert_eq!(output.summary.open_actionable_gaps, 0);
        assert!(card.next_action.summary.contains("Known baseline"));
        Ok(())
    }

    #[test]
    fn suppression_policy_marks_exact_card_identity_as_suppressed() -> Result<(), String> {
        let root =
            copy_fixture_to_temp("raw_pointer_alignment", "unsafe-review-suppression-match")?;
        let card_id = single_card("raw_pointer_alignment", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_policy_ledger(
            &root,
            "unsafe-review-suppressions.toml",
            &card_id,
            "expires",
        )?;

        let output = fixture_output_at(&root)?;
        let card = single_card("raw_pointer_alignment suppression", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert_eq!(card.class, ReviewClass::Suppressed);
        assert_eq!(card.priority, Priority::Low);
        assert_eq!(output.summary.open_actionable_gaps, 0);
        assert!(card.next_action.summary.contains("Suppressed"));
        Ok(())
    }

    #[test]
    fn imported_receipt_marks_witness_evidence_present_by_exact_card_identity() -> Result<(), String>
    {
        let root = copy_fixture_to_temp("raw_pointer_alignment", "unsafe-review-receipt-match")?;
        let card_id = single_card("raw_pointer_alignment", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_receipt(&root, &card_id)?;

        let output = fixture_output_at(&root)?;
        let card = single_card("raw_pointer_alignment receipt", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert!(card.witness.present);
        assert!(card.witness.summary.contains("miri"));
        assert!(card.witness.summary.contains("ran"));
        assert!(!card.missing.iter().any(|missing| missing.kind == "witness"));
        assert!(
            card.obligation_evidence
                .iter()
                .all(|evidence| evidence.witness.present)
        );
        Ok(())
    }

    #[test]
    fn configured_receipt_keeps_witness_gap_visible() -> Result<(), String> {
        let root =
            copy_fixture_to_temp("raw_pointer_alignment", "unsafe-review-configured-receipt")?;
        let card_id = single_card("raw_pointer_alignment", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_receipt_with_strength(&root, &card_id, "configured")?;

        let output = fixture_output_at(&root)?;
        let card = single_card("raw_pointer_alignment configured receipt", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert!(!card.witness.present);
        assert!(card.witness.summary.contains("configured"));
        assert!(card.missing.iter().any(|missing| missing.kind == "witness"));
        assert!(
            card.obligation_evidence
                .iter()
                .all(|evidence| !evidence.witness.present)
        );
        Ok(())
    }

    #[test]
    fn wrong_tool_receipt_keeps_witness_gap_visible() -> Result<(), String> {
        let root =
            copy_fixture_to_temp("raw_pointer_alignment", "unsafe-review-wrong-tool-receipt")?;
        let card_id = single_card("raw_pointer_alignment", &fixture_output_at(&root)?)?
            .id
            .0
            .clone();
        write_receipt_with_tool_and_strength(&root, &card_id, "loom", "ran")?;

        let output = fixture_output_at(&root)?;
        let card = single_card("raw_pointer_alignment wrong-tool receipt", &output)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"))?;
        assert!(!card.witness.present);
        assert!(
            card.witness
                .summary
                .contains("does not match routed witness tools")
        );
        assert!(card.witness.summary.contains("loom"));
        assert!(card.missing.iter().any(|missing| missing.kind == "witness"));
        assert!(
            card.obligation_evidence
                .iter()
                .all(|evidence| !evidence.witness.present)
        );
        Ok(())
    }

    #[test]
    fn receipted_fixture_keeps_static_guard_gap_visible() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment_receipted")?;
        let card = single_card("raw_pointer_alignment_receipted", &output)?;

        assert_eq!(card.operation.family, OperationFamily::RawPointerRead);
        assert_eq!(card.class, ReviewClass::GuardMissing);
        assert!(card.witness.present);
        assert!(card.witness.summary.contains("miri"));
        assert!(card.witness.summary.contains("ran"));
        assert!(!card.missing.iter().any(|missing| missing.kind == "witness"));
        assert!(card.missing.iter().any(|missing| missing.kind == "guard"));
        assert!(
            !obligation_discharge_present(card, "alignment"),
            "Miri receipt evidence must not discharge the static alignment guard obligation"
        );
        assert!(
            card.obligation_evidence
                .iter()
                .all(|evidence| evidence.witness.present)
        );
        Ok(())
    }

    fn fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
        let root = fixture_root(name);
        fixture_output_at(&root)
    }

    fn fixture_output_at(root: &Path) -> Result<AnalyzeOutput, String> {
        analyze(AnalyzeInput {
            root: root.to_path_buf(),
            scope: Scope::Diff,
            diff: DiffSource::File(root.join("change.diff")),
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })
    }

    fn fixture_root(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    fn single_card<'a>(fixture: &str, output: &'a AnalyzeOutput) -> Result<&'a ReviewCard, String> {
        if output.cards.len() != 1 {
            return Err(format!(
                "{fixture} should emit exactly one card, got {}",
                output.cards.len()
            ));
        }
        Ok(&output.cards[0])
    }

    fn temp_source_output(prefix: &str, source: &str) -> Result<AnalyzeOutput, String> {
        let root = unique_temp_dir(prefix)?;
        fs::create_dir_all(root.join("src"))
            .map_err(|err| format!("create temp fixture failed: {err}"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"js-buffer-reentry-fixture\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
        )
        .map_err(|err| format!("write temp Cargo.toml failed: {err}"))?;
        fs::write(root.join("src/lib.rs"), source)
            .map_err(|err| format!("write temp source failed: {err}"))?;

        let output = analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Repo,
            diff: DiffSource::NoneRepoScan,
            mode: AnalysisMode::Repo,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        });
        let cleanup =
            fs::remove_dir_all(&root).map_err(|err| format!("remove temp fixture failed: {err}"));
        match (output, cleanup) {
            (Ok(output), Ok(())) => Ok(output),
            (Err(err), _) => Err(err),
            (Ok(_), Err(err)) => Err(err),
        }
    }

    fn obligation_discharge_present(card: &ReviewCard, key: &str) -> bool {
        card.obligation_evidence
            .iter()
            .find(|evidence| evidence.obligation.key == key)
            .is_some_and(|evidence| evidence.discharge.present)
    }

    fn assert_no_unknown_wrapper_card(fixture: &str, output: &AnalyzeOutput) {
        assert!(
            output.cards.iter().all(|card| {
                !(card.site.kind == UnsafeSiteKind::UnsafeBlock
                    && card.operation.family == OperationFamily::Unknown)
            }),
            "{fixture} should suppress duplicate unknown unsafe-block wrapper cards"
        );
    }

    fn identity_without_count(id: &CardId) -> &str {
        id.0.rsplit_once("-c")
            .map_or(id.0.as_str(), |(base, _count)| base)
    }

    fn copy_fixture_to_temp(name: &str, prefix: &str) -> Result<PathBuf, String> {
        let source = fixture_root(name);
        let target = unique_temp_dir(prefix)?;
        copy_dir_all(&source, &target)?;
        Ok(target)
    }

    fn copy_dir_all(source: &Path, target: &Path) -> Result<(), String> {
        fs::create_dir_all(target)
            .map_err(|err| format!("create {} failed: {err}", target.display()))?;
        let entries = fs::read_dir(source)
            .map_err(|err| format!("read {} failed: {err}", source.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
            let source_path = entry.path();
            let target_path = target.join(entry.file_name());
            if source_path.is_dir() {
                copy_dir_all(&source_path, &target_path)?;
            } else {
                fs::copy(&source_path, &target_path).map_err(|err| {
                    format!(
                        "copy {} to {} failed: {err}",
                        source_path.display(),
                        target_path.display()
                    )
                })?;
            }
        }
        Ok(())
    }

    fn write_policy_ledger(
        root: &Path,
        file_name: &str,
        card_id: &str,
        date_key: &str,
    ) -> Result<(), String> {
        let policy_dir = root.join("policy");
        fs::create_dir_all(&policy_dir)
            .map_err(|err| format!("create {} failed: {err}", policy_dir.display()))?;
        fs::write(
            policy_dir.join(file_name),
            format!(
                r#"schema_version = "0.1"
status = "active"

[[entries]]
card_id = "{card_id}"
owner = "core/policy"
reason = "fixture policy match"
evidence = "test fixture"
{date_key} = "2026-08-01"
"#
            ),
        )
        .map_err(|err| format!("write policy ledger failed: {err}"))
    }

    fn write_receipt(root: &Path, card_id: &str) -> Result<(), String> {
        write_receipt_with_strength(root, card_id, "ran")
    }

    fn write_receipt_with_strength(
        root: &Path,
        card_id: &str,
        strength: &str,
    ) -> Result<(), String> {
        write_receipt_with_tool_and_strength(root, card_id, "miri", strength)
    }

    fn write_receipt_with_tool_and_strength(
        root: &Path,
        card_id: &str,
        tool: &str,
        strength: &str,
    ) -> Result<(), String> {
        let receipt_dir = root.join(".unsafe-review").join("receipts");
        fs::create_dir_all(&receipt_dir)
            .map_err(|err| format!("create {} failed: {err}", receipt_dir.display()))?;
        fs::write(
            receipt_dir.join("miri.json"),
            format!(
                r#"{{
  "schema_version": "0.1",
  "card_id": "{card_id}",
  "tool": "{tool}",
  "strength": "{strength}",
  "author": "core/fixtures",
  "recorded_at": "2026-05-18T00:00:00Z",
  "expires_at": "2026-08-18",
  "summary": "focused fixture witness passed",
  "command": "cargo +nightly miri test read_header",
  "limitations": ["fixture only"]
}}"#
            ),
        )
        .map_err(|err| format!("write receipt failed: {err}"))
    }

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }
}
