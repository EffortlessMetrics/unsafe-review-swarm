use super::{card_builder, progress::RepoScanReporter};
use crate::analysis::{receipts, scanner};
use crate::domain::ReviewCard;
use crate::input::diff::DiffIndex;
use crate::policy::PolicyState;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) struct ScanInputs<'a> {
    pub(super) root: &'a Path,
    pub(super) package: &'a str,
    pub(super) receipt_index: &'a receipts::ReceiptIndex,
    pub(super) policy_state: &'a PolicyState,
    pub(super) diff_index: &'a DiffIndex,
    pub(super) repo_mode: bool,
    pub(super) discovered_files: usize,
    pub(super) max_cards: usize,
}

pub(super) struct ScanResult {
    pub(super) cards: Vec<ReviewCard>,
    pub(super) files_scanned: usize,
    pub(super) last_scanned_path: Option<PathBuf>,
}

pub(super) fn scan_candidate_files(
    candidate_files: &[PathBuf],
    inputs: ScanInputs<'_>,
    reporter: &mut RepoScanReporter<'_>,
) -> Result<ScanResult, String> {
    let mut state = ScanState::new(inputs.max_cards);
    reporter.emit_scanning(
        inputs.discovered_files,
        state.files_scanned,
        state.cards.len(),
        None,
    )?;

    for rel in candidate_files {
        if state.cards.len() >= state.max_cards {
            break;
        }
        scan_one_candidate(rel, &inputs, &mut state, reporter)?;
    }

    state.cards.sort_by(|left, right| {
        left.site
            .location
            .file
            .cmp(&right.site.location.file)
            .then(left.site.location.line.cmp(&right.site.location.line))
    });

    Ok(ScanResult {
        cards: state.cards,
        files_scanned: state.files_scanned,
        last_scanned_path: state.last_scanned_path,
    })
}

struct ScanState {
    cards: Vec<ReviewCard>,
    identity_counts: BTreeMap<String, usize>,
    files_scanned: usize,
    last_scanned_path: Option<PathBuf>,
    max_cards: usize,
}

impl ScanState {
    fn new(max_cards: usize) -> Self {
        Self {
            cards: Vec::new(),
            identity_counts: BTreeMap::new(),
            files_scanned: 0,
            last_scanned_path: None,
            max_cards,
        }
    }
}

fn scan_one_candidate(
    rel: &PathBuf,
    inputs: &ScanInputs<'_>,
    state: &mut ScanState,
    reporter: &mut RepoScanReporter<'_>,
) -> Result<(), String> {
    reporter.emit_scanning(
        inputs.discovered_files,
        state.files_scanned,
        state.cards.len(),
        Some(rel.clone()),
    )?;

    let scanned = scanner::scan_file(inputs.root, rel, Some(inputs.diff_index), inputs.repo_mode)?;
    state.files_scanned += 1;
    append_cards(scanned, inputs, state);

    reporter.emit_scanning(
        inputs.discovered_files,
        state.files_scanned,
        state.cards.len(),
        Some(rel.clone()),
    )?;
    state.last_scanned_path = Some(rel.clone());
    Ok(())
}

fn append_cards(
    scanned_sites: Vec<scanner::ScannedSite>,
    inputs: &ScanInputs<'_>,
    state: &mut ScanState,
) {
    let max_cards = state.max_cards;
    let mut build_ctx = card_builder::CardBuildContext {
        root: inputs.root,
        package: inputs.package,
        receipt_index: inputs.receipt_index,
        policy_state: inputs.policy_state,
        identity_counts: &mut state.identity_counts,
    };

    for scanned_site in scanned_sites {
        if state.cards.len() >= max_cards {
            break;
        }
        state
            .cards
            .push(card_builder::build_card(&mut build_ctx, scanned_site));
    }
}
