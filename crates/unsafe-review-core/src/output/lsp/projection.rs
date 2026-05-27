use crate::api::{AnalyzeOutput, Scope};
use crate::domain::{EvidenceState, ObligationEvidence, Priority, ReviewCard, WitnessRoute};
use crate::util::path_display;
use serde::{Deserialize, Serialize};

const TRUST_BOUNDARY: &str = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.";

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
            code_actions: output.cards.iter().flat_map(code_actions).collect(),
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
            contents: hover_contents(card),
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

fn code_actions(card: &ReviewCard) -> Vec<LspCodeAction<'_>> {
    let path = path_display(&card.site.location.file);
    let range = range_for(card);
    let mut actions = vec![
        LspCodeAction {
            card_id: &card.id.0,
            path: path.clone(),
            range: range.clone(),
            title: format!("Copy unsafe-review packet for {}", card.id.0),
            kind: "quickfix",
            command: "unsafe-review.copyAgentPacket",
            payload: LspCodeActionPayload {
                kind: "unsafe-review.agent_packet",
                card_id: &card.id.0,
                file: None,
                line: None,
                name: None,
                command: None,
                trust_boundary: TRUST_BOUNDARY,
            },
            arguments: vec![card.id.0.clone()],
        },
        LspCodeAction {
            card_id: &card.id.0,
            path: path.clone(),
            range: range.clone(),
            title: "Explain unsafe-review witness route".to_string(),
            kind: "quickfix",
            command: "unsafe-review.explainWitnessRoute",
            payload: LspCodeActionPayload {
                kind: "unsafe-review.witness_route",
                card_id: &card.id.0,
                file: None,
                line: None,
                name: None,
                command: None,
                trust_boundary: TRUST_BOUNDARY,
            },
            arguments: vec![card.id.0.clone()],
        },
    ];
    if let Some(test) = card.related_tests.first() {
        actions.push(LspCodeAction {
            card_id: &card.id.0,
            path: test.file.clone(),
            range: LspRange {
                start: LspPosition {
                    line: test.line.saturating_sub(1),
                    character: 0,
                },
                end: LspPosition {
                    line: test.line.saturating_sub(1),
                    character: 1,
                },
            },
            title: format!("Open related test {}", test.name),
            kind: "quickfix",
            command: "unsafe-review.openRelatedTest",
            payload: LspCodeActionPayload {
                kind: "unsafe-review.related_test",
                card_id: &card.id.0,
                file: Some(&test.file),
                line: Some(test.line),
                name: Some(&test.name),
                command: None,
                trust_boundary: TRUST_BOUNDARY,
            },
            arguments: vec![
                card.id.0.clone(),
                test.file.clone(),
                test.line.to_string(),
                test.name.clone(),
            ],
        });
    }
    if let Some(command) = card.next_action.verify_commands.first() {
        actions.push(LspCodeAction {
            card_id: &card.id.0,
            path,
            range,
            title: "Copy witness command (does not run)".to_string(),
            kind: "quickfix",
            command: "unsafe-review.copyWitnessCommand",
            payload: LspCodeActionPayload {
                kind: "unsafe-review.witness_command",
                card_id: &card.id.0,
                file: None,
                line: None,
                name: None,
                command: Some(command),
                trust_boundary: TRUST_BOUNDARY,
            },
            arguments: vec![command.clone()],
        });
    }
    actions
}

fn hover_contents(card: &ReviewCard) -> String {
    let mut text = String::new();
    text.push_str(&format!(
        "Card: `{}`; priority `{}`; confidence `{}`\n\n",
        card.id,
        card.priority.as_str(),
        card.confidence.as_str()
    ));
    text.push_str("Why this card exists:\n");
    text.push_str(&format!(
        "- The changed code contains a `{}` unsafe operation that unsafe-review classifies as `{}`.\n",
        card.operation.family.as_str(),
        card.class.as_str()
    ));
    text.push_str(&format!("- Operation: `{}`\n\n", card.operation.expression));
    if !card.hazards.is_empty() {
        text.push_str("Relevant hazard families:\n");
        for hazard in &card.hazards {
            text.push_str(&format!("- `{}`\n", hazard.as_str()));
        }
        text.push('\n');
    }
    text.push_str("Required safety conditions:\n");
    for obligation in &card.obligations {
        text.push_str(&format!("- {}\n", obligation.description));
    }
    text.push_str("\nEvidence found:\n");
    text.push_str(&format!(
        "- Contract [{}]: {}\n",
        present_label(card.contract.present),
        card.contract.summary
    ));
    text.push_str(&format!(
        "- Guard/discharge [{}]: {}\n",
        present_label(card.discharge.present),
        card.discharge.summary
    ));
    text.push_str(&format!(
        "- Reach [{}]: {}\n",
        card.reach.state, card.reach.summary
    ));
    text.push_str(&format!(
        "- Witness [{}]: {}\n",
        present_label(card.witness.present),
        card.witness.summary
    ));
    text.push_str("\nEvidence missing:\n");
    if card.missing.is_empty() {
        text.push_str("- none recorded\n");
    } else {
        for missing in &card.missing {
            text.push_str(&format!("- {}\n", missing.message));
        }
    }
    text.push_str("\nWhat would resolve this:\n");
    text.push_str(&format!("- {}\n", card.next_action.summary));
    if !card.next_action.verify_commands.is_empty() {
        text.push_str("\nVerify commands:\n");
        for command in &card.next_action.verify_commands {
            text.push_str(&format!("- `{command}`\n"));
        }
    }
    text.push_str("\nWhat would not resolve this:\n");
    text.push_str("- A `SAFETY:` comment alone does not discharge missing guard evidence.\n");
    text.push_str("- A related test mention is not proof that this unsafe site executed.\n");
    text.push_str("- Do not claim witness proof unless a matching receipt exists.\n");
    text.push_str("- Do not widen unsafe scope, suppress the card, or change unrelated unsafe code to silence this review item.\n");
    if let Some(route) = card.routes.first() {
        text.push_str(&format!(
            "\nWitness route: `{}` because {}.\n",
            route.kind.as_str(),
            route.reason
        ));
    }
    text.push_str(
        "\nReach note: static related-test evidence does not prove the unsafe site executed.\n",
    );
    text.push_str("\nTrust boundary: ");
    text.push_str(TRUST_BOUNDARY);
    text
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

fn severity_for(card: &ReviewCard) -> usize {
    if matches!(card.priority, Priority::High) {
        2
    } else {
        3
    }
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
