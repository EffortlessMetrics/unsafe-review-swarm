use crate::api::{AnalyzeOutput, Scope};
use crate::domain::{
    CommentPlanStatus, CoverageBlock, EvidenceState, ObligationEvidence, Priority, ReviewCard,
    WitnessRoute,
};
use crate::freshness::AnalysisIdentity;
use crate::output::{
    REVIEWCARD_TRUST_BOUNDARY as TRUST_BOUNDARY, agent::card_has_scoped_repairs, comment_plan,
};
use crate::policy::SnapshotCoverage;
use crate::util::path_display;
use serde::{Deserialize, Serialize};

mod hover;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorProjection {
    pub analysis: AnalysisIdentity,
    pub schema_version: String,
    pub tool: String,
    pub mode: String,
    pub policy: String,
    pub scope: String,
    pub status: EditorStatus,
    pub diagnostics: Vec<EditorDiagnostic>,
    pub hovers: Vec<EditorHover>,
    pub code_actions: Vec<super::EditorActionContract>,
    pub trust_boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorStatus {
    pub state: String,
    pub cards: usize,
    pub open_actionable_gaps: usize,
    pub high_priority_cards: usize,
    pub message: String,
    pub trust_boundary: String,
}

pub type EditorHover = serde_json::Value;

/// Render the rich hover markdown for a single [`ReviewCard`].
///
/// This is the same content as `lsp.json` `hovers[].contents`: obligations,
/// evidence state, hazard families, verify commands, witness route, handoff
/// commands, and the advisory trust boundary.
pub(crate) fn render_hover(card: &ReviewCard) -> String {
    hover::contents(card)
}

pub(crate) fn project_editor(output: &AnalyzeOutput) -> EditorProjection {
    let projection = LspProjection::from(output);
    EditorProjection {
        analysis: projection.analysis.clone(),
        schema_version: projection.schema_version.to_string(),
        tool: projection.tool.to_string(),
        mode: projection.mode.to_string(),
        policy: projection.policy.to_string(),
        scope: projection.scope.to_string(),
        status: EditorStatus {
            state: projection.status.state.to_string(),
            cards: projection.status.cards,
            open_actionable_gaps: projection.status.open_actionable_gaps,
            high_priority_cards: projection.status.high_priority_cards,
            message: projection.status.message,
            trust_boundary: projection.status.trust_boundary.to_string(),
        },
        diagnostics: projection.diagnostics,
        hovers: projection
            .hovers
            .iter()
            .map(|item| serde_json::to_value(item).unwrap_or(serde_json::Value::Null))
            .collect(),
        code_actions: projection.code_actions,
        trust_boundary: projection.trust_boundary.to_string(),
    }
}

pub(crate) fn project_editor_diagnostics(output: &AnalyzeOutput) -> Vec<EditorDiagnostic> {
    diagnostics_for(output)
}

pub(crate) fn project_actionable_editor_diagnostics(
    output: &AnalyzeOutput,
) -> Vec<EditorDiagnostic> {
    let statuses = comment_plan::card_statuses(output);
    diagnostics_for_with_status(output, &statuses, true)
}

#[derive(Serialize)]
struct LspProjection<'a> {
    analysis: &'a AnalysisIdentity,
    schema_version: &'static str,
    tool: &'a str,
    mode: &'static str,
    policy: &'static str,
    scope: &'static str,
    status: LspStatus,
    diagnostics: Vec<EditorDiagnostic>,
    hovers: Vec<LspHover<'a>>,
    code_actions: Vec<super::EditorActionContract>,
    trust_boundary: &'static str,
}

#[allow(
    clippy::panic,
    reason = "a card already owned by AnalyzeOutput must have a canonical diagnostic and action projection; silently dropping it would corrupt the saved artifact"
)]
impl<'a> From<&'a AnalyzeOutput> for LspProjection<'a> {
    fn from(output: &'a AnalyzeOutput) -> Self {
        Self {
            analysis: &output.analysis_identity,
            schema_version: "0.2",
            tool: &output.tool,
            mode: "read_only_projection",
            policy: output.policy.as_str(),
            scope: scope_label(output),
            status: status_for(output),
            diagnostics: diagnostics_for(output),
            hovers: output
                .cards
                .iter()
                .map(|card| LspHover::from_card(card, &output.analysis_identity))
                .collect(),
            code_actions: output
                .cards
                .iter()
                .flat_map(|card| {
                    super::actions_for_card(output, &card.id.0).unwrap_or_else(|error| {
                        panic!(
                            "canonical saved LSP actions must project for card `{}`: {error}",
                            card.id.0
                        )
                    })
                })
                .collect(),
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

fn diagnostics_for(output: &AnalyzeOutput) -> Vec<EditorDiagnostic> {
    let statuses = comment_plan::card_statuses(output);
    diagnostics_for_with_status(output, &statuses, false)
}

fn diagnostics_for_with_status(
    output: &AnalyzeOutput,
    statuses: &std::collections::HashMap<crate::domain::CardId, CommentPlanStatus>,
    actionable_only: bool,
) -> Vec<EditorDiagnostic> {
    output
        .cards
        .iter()
        .filter(|card| !actionable_only || card.class.is_actionable())
        .map(|card| {
            let status = statuses
                .get(&card.id)
                .copied()
                .unwrap_or(CommentPlanStatus::NotEligible);
            let snapshot = output.coverage_snapshot.get(&card.id.0);
            EditorDiagnostic::from_with_status(card, status, snapshot)
        })
        .collect()
}

/// Canonical, card-scoped diagnostic data for editor and agent projections.
///
/// This owned DTO deliberately contains review semantics rather than LSP
/// transport details.  Saved LSP, live LSP, VS Code, and agent adapters can
/// consume the same fields without independently deriving class, range,
/// evidence, or readiness.  The DTO remains read-only and advisory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorDiagnostic {
    pub card_id: String,
    pub code: String,
    pub coverage: EditorCoverageBlock,
    pub evidence_summary: EditorEvidenceSummary,
    pub hazards: Vec<String>,
    pub message: String,
    pub missing_evidence: Vec<String>,
    pub next_action: String,
    pub obligation_evidence: Vec<EditorObligationEvidence>,
    pub operation: String,
    pub operation_family: String,
    pub path: String,
    pub proof_path: String,
    pub range: EditorRange,
    pub required_safety_conditions: Vec<EditorSafetyCondition>,
    pub severity: usize,
    pub source: String,
    pub trust_boundary: String,
    pub verify_commands: Vec<String>,
    pub witness_routes: Vec<EditorWitnessRoute>,
}

impl EditorDiagnostic {
    pub(crate) fn from_with_status(
        card: &ReviewCard,
        comment_plan_status: CommentPlanStatus,
        snapshot: Option<&SnapshotCoverage>,
    ) -> Self {
        let mut coverage_block = card.coverage_block();
        coverage_block.comment_plan_status = comment_plan_status;
        if let Some(snap) = snapshot {
            coverage_block.apply_snapshot_slots(
                &snap.contract_coverage,
                &snap.guard_coverage,
                &snap.test_reach_coverage,
                &snap.witness_receipt_coverage,
            );
        }
        coverage_block.agent_lsp_readiness = crate::domain::coverage::compute_agent_lsp_readiness(
            card,
            card_has_scoped_repairs(card),
        )
        .state;
        Self {
            card_id: card.id.0.clone(),
            code: card.class.as_str().to_string(),
            coverage: EditorCoverageBlock::from(coverage_block),
            evidence_summary: EditorEvidenceSummary::from(card),
            hazards: card
                .hazards
                .iter()
                .map(|hazard| hazard.as_str().to_string())
                .collect(),
            message: format!(
                "{}: {}",
                card.operation.family.as_str(),
                card.next_action.summary
            ),
            missing_evidence: card
                .missing
                .iter()
                .map(|missing| missing.message.clone())
                .collect(),
            next_action: card.next_action.summary.clone(),
            obligation_evidence: card
                .obligation_evidence
                .iter()
                .map(EditorObligationEvidence::from)
                .collect(),
            operation: card.operation.expression.clone(),
            operation_family: card.operation.family.as_str().to_string(),
            path: path_display(&card.site.location.file),
            proof_path: card.proof_path.as_str().to_string(),
            range: range_for(card),
            required_safety_conditions: card
                .obligations
                .iter()
                .map(|obligation| EditorSafetyCondition {
                    key: obligation.key.clone(),
                    description: obligation.description.clone(),
                })
                .collect(),
            severity: severity_for(card),
            source: "unsafe-review".to_string(),
            trust_boundary: TRUST_BOUNDARY.to_string(),
            verify_commands: card.next_action.verify_commands.clone(),
            witness_routes: card.routes.iter().map(EditorWitnessRoute::from).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorCoverageBlock {
    pub agent_lsp_readiness: String,
    pub baseline_state: String,
    pub comment_plan_status: String,
    pub contract_coverage: String,
    pub guard_coverage: String,
    pub manual_context: String,
    pub outcome_movement: String,
    pub test_reach_coverage: String,
    pub witness_receipt_coverage: String,
}

impl From<CoverageBlock> for EditorCoverageBlock {
    fn from(block: CoverageBlock) -> Self {
        Self {
            contract_coverage: block.contract_coverage.as_str().to_string(),
            guard_coverage: block.guard_coverage.as_str().to_string(),
            test_reach_coverage: block.test_reach_coverage.as_str().to_string(),
            witness_receipt_coverage: block.witness_receipt_coverage.as_str().to_string(),
            manual_context: block.manual_context.as_str().to_string(),
            baseline_state: block.baseline_state.as_str().to_string(),
            outcome_movement: block.outcome_movement.as_str().to_string(),
            comment_plan_status: block.comment_plan_status.as_str().to_string(),
            agent_lsp_readiness: block.agent_lsp_readiness.as_str().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSafetyCondition {
    pub description: String,
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorEvidenceSummary {
    pub contract: EditorSimpleEvidence,
    pub discharge: EditorSimpleEvidence,
    pub reach: EditorReachEvidence,
    pub reach_limitation: String,
    pub witness: EditorSimpleEvidence,
}

impl From<&ReviewCard> for EditorEvidenceSummary {
    fn from(card: &ReviewCard) -> Self {
        Self {
            contract: EditorSimpleEvidence {
                present: card.contract.present,
                state: present_label(card.contract.present).to_string(),
                summary: card.contract.summary.clone(),
            },
            discharge: EditorSimpleEvidence {
                present: card.discharge.present,
                state: present_label(card.discharge.present).to_string(),
                summary: card.discharge.summary.clone(),
            },
            reach: EditorReachEvidence {
                state: card.reach.state.clone(),
                summary: card.reach.summary.clone(),
            },
            witness: EditorSimpleEvidence {
                present: card.witness.present,
                state: present_label(card.witness.present).to_string(),
                summary: card.witness.summary.clone(),
            },
            reach_limitation: "static reach evidence is not proof that the unsafe site executed"
                .to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorSimpleEvidence {
    pub present: bool,
    pub state: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorReachEvidence {
    pub state: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorObligationEvidence {
    pub contract: EditorEvidenceState,
    pub description: String,
    pub discharge: EditorEvidenceState,
    pub key: String,
    pub reach: EditorEvidenceState,
    pub witness: EditorEvidenceState,
}

impl From<&ObligationEvidence> for EditorObligationEvidence {
    fn from(evidence: &ObligationEvidence) -> Self {
        Self {
            key: evidence.obligation.key.clone(),
            description: evidence.obligation.description.clone(),
            contract: EditorEvidenceState::from(&evidence.contract),
            discharge: EditorEvidenceState::from(&evidence.discharge),
            reach: EditorEvidenceState::from(&evidence.reach),
            witness: EditorEvidenceState::from(&evidence.witness),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorEvidenceState {
    pub present: bool,
    pub state: String,
    pub summary: String,
}

impl From<&EvidenceState> for EditorEvidenceState {
    fn from(state: &EvidenceState) -> Self {
        Self {
            present: state.present,
            state: state.state.clone(),
            summary: state.summary.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorWitnessRoute {
    pub command: Option<String>,
    pub kind: String,
    pub reason: String,
    pub required: bool,
}

impl From<&WitnessRoute> for EditorWitnessRoute {
    fn from(route: &WitnessRoute) -> Self {
        Self {
            kind: route.kind.as_str().to_string(),
            reason: route.reason.clone(),
            command: route.command.clone(),
            required: route.required,
        }
    }
}

#[derive(Serialize)]
struct LspHover<'a> {
    analysis: &'a AnalysisIdentity,
    card_id: &'a str,
    path: String,
    position: EditorPosition,
    range: EditorRange,
    contents: String,
    trust_boundary: &'static str,
}

impl<'a> LspHover<'a> {
    fn from_card(card: &'a ReviewCard, analysis: &'a AnalysisIdentity) -> Self {
        Self {
            analysis,
            card_id: &card.id.0,
            path: path_display(&card.site.location.file),
            position: position_for(card),
            range: range_for(card),
            contents: hover::contents(card),
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
struct LspStatus {
    state: &'static str,
    cards: usize,
    open_actionable_gaps: usize,
    high_priority_cards: usize,
    message: String,
    trust_boundary: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorRange {
    pub end: EditorPosition,
    pub start: EditorPosition,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditorPosition {
    pub character: usize,
    pub line: usize,
}

fn present_label(present: bool) -> &'static str {
    if present { "present" } else { "missing" }
}

pub(crate) fn range_for(card: &ReviewCard) -> EditorRange {
    let start = position_for(card);
    let end = EditorPosition {
        line: start.line,
        character: start
            .character
            .saturating_add(utf16_width(&card.site.snippet).max(1)),
    };
    EditorRange { start, end }
}

fn position_for(card: &ReviewCard) -> EditorPosition {
    EditorPosition {
        line: card.site.location.line.saturating_sub(1),
        character: card.site.location.column.saturating_sub(1),
    }
}

/// Return the number of UTF-16 code units in `text`, as required by LSP
/// positions.  Keeping this conversion beside the canonical editor range
/// prevents saved projections and their future adapters from counting scalar
/// values or bytes independently.
pub(crate) fn utf16_width(text: &str) -> usize {
    text.encode_utf16().count()
}

/// Return the LSP `DiagnosticSeverity` integer derived from the card's class.
///
/// Severity encodes "what kind of review concern is this?" and must agree
/// with the SARIF `level` emitted by the same card (both derive from
/// [`ReviewClass::lsp_severity`] / [`ReviewClass::sarif_level`]).
///
/// Priority is a ranking/ordering/budget signal only and is intentionally
/// NOT used here.
fn severity_for(card: &ReviewCard) -> usize {
    card.class.lsp_severity()
}

fn status_for(output: &AnalyzeOutput) -> LspStatus {
    let high_priority_cards = output
        .cards
        .iter()
        .filter(|card| matches!(card.priority, Priority::High))
        .count();
    let state = if output.cards.is_empty() {
        "quiet"
    } else if output.summary.open_actionable_gaps > 0 {
        "actionable"
    } else {
        "informational"
    };
    let message = match state {
        "quiet" => "No unsafe-review cards for this scope".to_string(),
        _ => format!(
            "{} unsafe-review card(s), {} open actionable gap(s)",
            output.summary.cards, output.summary.open_actionable_gaps
        ),
    };
    LspStatus {
        state,
        cards: output.summary.cards,
        open_actionable_gaps: output.summary.open_actionable_gaps,
        high_priority_cards,
        message,
        trust_boundary: TRUST_BOUNDARY,
    }
}

fn scope_label(output: &AnalyzeOutput) -> &'static str {
    match output.scope {
        Scope::Diff => "diff",
        Scope::Repo => "repo",
    }
}
