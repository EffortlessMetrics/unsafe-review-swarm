use crate::api::{AnalyzeOutput, Scope};
use crate::domain::{EvidenceState, ObligationEvidence, Priority, ReviewCard, WitnessRoute};
use crate::output::REVIEWCARD_TRUST_BOUNDARY as TRUST_BOUNDARY;
use crate::util::path_display;
use serde::{Deserialize, Serialize};

mod code_actions;
mod hover;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorProjection {
    pub schema_version: String,
    pub tool: String,
    pub mode: String,
    pub policy: String,
    pub scope: String,
    pub status: EditorStatus,
    pub diagnostics: Vec<EditorDiagnostic>,
    pub hovers: Vec<EditorHover>,
    pub code_actions: Vec<EditorCodeAction>,
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

pub type EditorDiagnostic = serde_json::Value;
pub type EditorHover = serde_json::Value;
pub type EditorCodeAction = serde_json::Value;

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
        diagnostics: projection
            .diagnostics
            .iter()
            .map(|item| serde_json::to_value(item).unwrap_or(serde_json::Value::Null))
            .collect(),
        hovers: projection
            .hovers
            .iter()
            .map(|item| serde_json::to_value(item).unwrap_or(serde_json::Value::Null))
            .collect(),
        code_actions: projection
            .code_actions
            .iter()
            .map(|item| serde_json::to_value(item).unwrap_or(serde_json::Value::Null))
            .collect(),
        trust_boundary: projection.trust_boundary.to_string(),
    }
}

#[derive(Serialize)]
struct LspProjection<'a> {
    schema_version: &'a str,
    tool: &'a str,
    mode: &'static str,
    policy: &'static str,
    scope: &'static str,
    status: LspStatus,
    diagnostics: Vec<LspDiagnostic<'a>>,
    hovers: Vec<LspHover<'a>>,
    code_actions: Vec<LspCodeAction<'a>>,
    trust_boundary: &'static str,
}

impl<'a> From<&'a AnalyzeOutput> for LspProjection<'a> {
    fn from(output: &'a AnalyzeOutput) -> Self {
        Self {
            schema_version: &output.schema_version,
            tool: &output.tool,
            mode: "read_only_projection",
            policy: output.policy.as_str(),
            scope: scope_label(output),
            status: status_for(output),
            diagnostics: output.cards.iter().map(LspDiagnostic::from).collect(),
            hovers: output.cards.iter().map(LspHover::from).collect(),
            code_actions: output
                .cards
                .iter()
                .flat_map(code_actions::for_card)
                .collect(),
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
struct LspDiagnostic<'a> {
    card_id: &'a str,
    path: String,
    range: LspRange,
    severity: usize,
    source: &'static str,
    code: &'static str,
    message: String,
    operation: &'a str,
    operation_family: &'static str,
    proof_path: &'static str,
    hazards: Vec<&'static str>,
    required_safety_conditions: Vec<LspSafetyCondition<'a>>,
    evidence_summary: LspEvidenceSummary<'a>,
    obligation_evidence: Vec<LspObligationEvidence<'a>>,
    missing_evidence: Vec<&'a str>,
    next_action: &'a str,
    witness_routes: Vec<LspWitnessRoute<'a>>,
    verify_commands: &'a [String],
    trust_boundary: &'static str,
}

impl<'a> From<&'a ReviewCard> for LspDiagnostic<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            card_id: &card.id.0,
            path: path_display(&card.site.location.file),
            range: range_for(card),
            severity: severity_for(card),
            source: "unsafe-review",
            code: card.class.as_str(),
            message: format!(
                "{}: {}",
                card.operation.family.as_str(),
                card.next_action.summary
            ),
            operation: &card.operation.expression,
            operation_family: card.operation.family.as_str(),
            proof_path: card.proof_path.as_str(),
            hazards: card.hazards.iter().map(|hazard| hazard.as_str()).collect(),
            required_safety_conditions: card
                .obligations
                .iter()
                .map(|obligation| LspSafetyCondition {
                    key: &obligation.key,
                    description: &obligation.description,
                })
                .collect(),
            evidence_summary: LspEvidenceSummary::from(card),
            obligation_evidence: card
                .obligation_evidence
                .iter()
                .map(LspObligationEvidence::from)
                .collect(),
            missing_evidence: card
                .missing
                .iter()
                .map(|missing| missing.message.as_str())
                .collect(),
            next_action: &card.next_action.summary,
            witness_routes: card.routes.iter().map(LspWitnessRoute::from).collect(),
            verify_commands: &card.next_action.verify_commands,
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
struct LspSafetyCondition<'a> {
    key: &'a str,
    description: &'a str,
}

#[derive(Serialize)]
struct LspEvidenceSummary<'a> {
    contract: LspSimpleEvidence<'a>,
    discharge: LspSimpleEvidence<'a>,
    reach: LspReachEvidence<'a>,
    witness: LspSimpleEvidence<'a>,
    reach_limitation: &'static str,
}

impl<'a> From<&'a ReviewCard> for LspEvidenceSummary<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            contract: LspSimpleEvidence {
                present: card.contract.present,
                state: present_label(card.contract.present),
                summary: &card.contract.summary,
            },
            discharge: LspSimpleEvidence {
                present: card.discharge.present,
                state: present_label(card.discharge.present),
                summary: &card.discharge.summary,
            },
            reach: LspReachEvidence {
                state: &card.reach.state,
                summary: &card.reach.summary,
            },
            witness: LspSimpleEvidence {
                present: card.witness.present,
                state: present_label(card.witness.present),
                summary: &card.witness.summary,
            },
            reach_limitation: "static reach evidence is not proof that the unsafe site executed",
        }
    }
}

#[derive(Serialize)]
struct LspSimpleEvidence<'a> {
    present: bool,
    state: &'static str,
    summary: &'a str,
}

#[derive(Serialize)]
struct LspReachEvidence<'a> {
    state: &'a str,
    summary: &'a str,
}

#[derive(Serialize)]
struct LspObligationEvidence<'a> {
    key: &'a str,
    description: &'a str,
    contract: LspEvidenceState<'a>,
    discharge: LspEvidenceState<'a>,
    reach: LspEvidenceState<'a>,
    witness: LspEvidenceState<'a>,
}

impl<'a> From<&'a ObligationEvidence> for LspObligationEvidence<'a> {
    fn from(evidence: &'a ObligationEvidence) -> Self {
        Self {
            key: &evidence.obligation.key,
            description: &evidence.obligation.description,
            contract: LspEvidenceState::from(&evidence.contract),
            discharge: LspEvidenceState::from(&evidence.discharge),
            reach: LspEvidenceState::from(&evidence.reach),
            witness: LspEvidenceState::from(&evidence.witness),
        }
    }
}

#[derive(Serialize)]
struct LspEvidenceState<'a> {
    present: bool,
    state: &'a str,
    summary: &'a str,
}

impl<'a> From<&'a EvidenceState> for LspEvidenceState<'a> {
    fn from(state: &'a EvidenceState) -> Self {
        Self {
            present: state.present,
            state: &state.state,
            summary: &state.summary,
        }
    }
}

#[derive(Serialize)]
struct LspWitnessRoute<'a> {
    kind: &'static str,
    reason: &'a str,
    command: Option<&'a str>,
    required: bool,
}

impl<'a> From<&'a WitnessRoute> for LspWitnessRoute<'a> {
    fn from(route: &'a WitnessRoute) -> Self {
        Self {
            kind: route.kind.as_str(),
            reason: &route.reason,
            command: route.command.as_deref(),
            required: route.required,
        }
    }
}

#[derive(Serialize)]
struct LspHover<'a> {
    card_id: &'a str,
    path: String,
    position: LspPosition,
    contents: String,
    trust_boundary: &'static str,
}

impl<'a> From<&'a ReviewCard> for LspHover<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            card_id: &card.id.0,
            path: path_display(&card.site.location.file),
            position: position_for(card),
            contents: hover::contents(card),
            trust_boundary: TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
struct LspCodeAction<'a> {
    card_id: &'a str,
    path: String,
    range: LspRange,
    title: String,
    kind: &'static str,
    command: &'static str,
    payload: LspCodeActionPayload<'a>,
    arguments: Vec<String>,
}

#[derive(Serialize)]
struct LspCodeActionPayload<'a> {
    kind: &'static str,
    card_id: &'a str,
    proof_path: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'a str>,
    trust_boundary: &'static str,
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

#[derive(Serialize, Clone)]
struct LspRange {
    start: LspPosition,
    end: LspPosition,
}

#[derive(Serialize, Clone)]
struct LspPosition {
    line: usize,
    character: usize,
}

fn present_label(present: bool) -> &'static str {
    if present { "present" } else { "missing" }
}

fn range_for(card: &ReviewCard) -> LspRange {
    let start = position_for(card);
    let end = LspPosition {
        line: start.line,
        character: start
            .character
            .saturating_add(card.site.snippet.chars().count().max(1)),
    };
    LspRange { start, end }
}

fn position_for(card: &ReviewCard) -> LspPosition {
    LspPosition {
        line: card.site.location.line.saturating_sub(1),
        character: card.site.location.column.saturating_sub(1),
    }
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
