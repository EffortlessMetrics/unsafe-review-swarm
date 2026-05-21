use crate::domain::{
    Confidence, EvidenceState, ObligationEvidence, OperationFamily, ReviewCard, ReviewClass,
    WitnessKind, WitnessRoute,
};
use crate::util::path_display;
use serde::Serialize;

const TRUST_BOUNDARY: &str = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.";

pub(crate) fn render(card: &ReviewCard) -> String {
    render_pretty(&AgentPacket::from(card))
}

fn render_pretty(value: &impl Serialize) -> String {
    match serde_json::to_string_pretty(value) {
        Ok(text) => text,
        Err(err) => format!("{{\n  \"error\": \"agent packet serialization failed: {err}\"\n}}"),
    }
}

#[derive(Serialize)]
struct AgentPacket<'a> {
    schema_version: &'static str,
    tool: &'static str,
    mode: &'static str,
    source: &'static str,
    policy: &'static str,
    trust_boundary: &'static str,
    card_id: &'a str,
    card: AgentCard<'a>,
    task: &'a str,
    context: AgentContext<'a>,
    safety_contract: AgentSafetyContract<'a>,
    required_safety_conditions: Vec<&'a str>,
    obligation_evidence: Vec<AgentObligationEvidence<'a>>,
    missing: Vec<&'a str>,
    missing_evidence: Vec<AgentMissingEvidence<'a>>,
    allowed_repairs: Vec<String>,
    agent_readiness: AgentReadiness,
    repair_scope: &'static str,
    witness_routes: Vec<AgentWitnessRoute<'a>>,
    verify_commands: &'a [String],
    do_not_do: &'static [&'static str],
    stop_conditions: &'static [&'static str],
}

impl<'a> From<&'a ReviewCard> for AgentPacket<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        let allowed_repairs = allowed_repairs(card);
        let agent_readiness = agent_readiness(card, allowed_repairs.has_card_scoped_repairs);
        Self {
            schema_version: "0.1",
            tool: "unsafe-review",
            mode: "bounded_repair_packet",
            source: "review_card",
            policy: "advisory",
            trust_boundary: TRUST_BOUNDARY,
            card_id: &card.id.0,
            card: AgentCard::from(card),
            task: &card.next_action.summary,
            context: AgentContext::from(card),
            safety_contract: AgentSafetyContract::from(card),
            required_safety_conditions: card
                .obligations
                .iter()
                .map(|obligation| obligation.description.as_str())
                .collect(),
            obligation_evidence: card
                .obligation_evidence
                .iter()
                .map(AgentObligationEvidence::from)
                .collect(),
            missing: card
                .missing
                .iter()
                .map(|missing| missing.message.as_str())
                .collect(),
            missing_evidence: card
                .missing
                .iter()
                .map(|missing| AgentMissingEvidence {
                    kind: &missing.kind,
                    message: &missing.message,
                })
                .collect(),
            allowed_repairs: allowed_repairs.repairs,
            agent_readiness,
            repair_scope: "this card only",
            witness_routes: card.routes.iter().map(AgentWitnessRoute::from).collect(),
            verify_commands: &card.next_action.verify_commands,
            do_not_do: &[
                "do not widen unsafe code without reducing the missing evidence",
                "do not add a broad suppression",
                "do not claim Miri proof unless the witness command is run and attached",
                "do not change unrelated unsafe code or public API behavior",
                "do not treat a test mention as proof that the unsafe site executed",
            ],
            stop_conditions: &[
                "the missing evidence is present or explicitly waived with owner and expiry",
                "the focused test or witness command has been run or marked unavailable",
                "no unrelated unsafe code was changed",
                "the ReviewCard identity still maps to the same unsafe seam",
            ],
        }
    }
}

fn agent_readiness(card: &ReviewCard, has_card_scoped_repairs: bool) -> AgentReadiness {
    let mut reasons = Vec::new();

    if !card.class.is_actionable() {
        reasons.push(format!(
            "card class `{}` is not an open actionable repair target",
            card.class.as_str()
        ));
        return AgentReadiness::not_ready("not_recommended", reasons);
    }

    if matches!(
        card.class,
        ReviewClass::StaticUnknown
            | ReviewClass::MiriUnsupported
            | ReviewClass::RequiresSanitizer
            | ReviewClass::RequiresKaniOrCrux
            | ReviewClass::RequiresLoom
    ) {
        reasons.push(format!(
            "card class `{}` requires specialist review or external witness routing",
            card.class.as_str()
        ));
    }

    if matches!(
        card.operation.family,
        OperationFamily::Unknown
            | OperationFamily::Ffi
            | OperationFamily::InlineAsm
            | OperationFamily::TargetFeature
    ) {
        reasons.push(format!(
            "operation family `{}` is not safe for automatic repair delegation",
            card.operation.family.as_str()
        ));
    }

    if card.routes.iter().any(|route| {
        matches!(
            route.kind,
            WitnessKind::HumanDeepReview | WitnessKind::Unsupported
        )
    }) {
        reasons.push("witness route requires human deep review or is unsupported".to_string());
    }

    if !matches!(card.confidence, Confidence::High | Confidence::Medium) {
        reasons.push(format!(
            "card confidence `{}` is too weak for bounded repair delegation",
            card.confidence.as_str()
        ));
    }

    if !has_card_scoped_repairs {
        reasons.push("no card-scoped allowed repair is available".to_string());
    }

    if card.next_action.verify_commands.is_empty() {
        reasons.push("no verify command is available for this card".to_string());
    }

    if reasons.is_empty() {
        AgentReadiness {
            ready: true,
            state: "ready",
            reasons: vec![
                "specific operation family".to_string(),
                "card-scoped allowed repairs".to_string(),
                "verify commands available".to_string(),
                "medium-or-high confidence".to_string(),
            ],
        }
    } else {
        AgentReadiness::not_ready("needs_human_review", reasons)
    }
}

struct AllowedRepairs {
    repairs: Vec<String>,
    has_card_scoped_repairs: bool,
}

fn allowed_repairs(card: &ReviewCard) -> AllowedRepairs {
    let mut repairs = Vec::new();

    match card.operation.family {
        OperationFamily::RawPointerDeref
        | OperationFamily::RawPointerRead
        | OperationFamily::RawPointerWrite => {
            add_raw_pointer_repairs(card, &mut repairs, true);
        }
        OperationFamily::RawPointerReadUnaligned | OperationFamily::RawPointerWriteUnaligned => {
            add_raw_pointer_repairs(card, &mut repairs, false);
        }
        OperationFamily::CopyNonOverlapping => {
            if missing_discharge(card, "valid-range") {
                repairs.push(
                    "add guards proving `count` fits both source and destination ranges before this copy"
                        .to_string(),
                );
            }
            if missing_discharge(card, "non-overlap") {
                repairs.push(
                    "prove source and destination ranges do not overlap, or use `ptr::copy` only if overlap is intended"
                        .to_string(),
                );
            }
        }
        OperationFamily::PtrCopy => {
            if missing_discharge(card, "valid-range") {
                repairs.push(
                    "add guards proving `count` fits both source and destination ranges before this copy"
                        .to_string(),
                );
            }
            if missing_discharge(card, "initialized") {
                repairs.push(
                    "show that the source range is initialized for the copied element count"
                        .to_string(),
                );
            }
        }
        OperationFamily::VecSetLen => {
            if missing_discharge(card, "capacity") {
                repairs.push(
                    "add a same-vector capacity guard before `set_len` for the requested length"
                        .to_string(),
                );
            }
            if missing_discharge(card, "initialized") {
                repairs.push(
                    "initialize the extended element range before calling `set_len`".to_string(),
                );
            }
        }
        OperationFamily::StrFromUtf8Unchecked if missing_discharge(card, "utf8") => {
            repairs.push(
                "validate the same byte buffer as UTF-8 on an open path before calling `from_utf8_unchecked`"
                    .to_string(),
            );
        }
        OperationFamily::NonNullUnchecked if missing_discharge(card, "non-null") => {
            repairs.push(
                "add a same-pointer non-null guard before `NonNull::new_unchecked`".to_string(),
            );
        }
        OperationFamily::UnsafeImplSendSync => {
            repairs.push(
                "document or add evidence for the thread-safety invariant of this unsafe impl"
                    .to_string(),
            );
            repairs.push(
                "route concurrency-sensitive evidence through Loom or Shuttle when the invariant depends on interleavings"
                    .to_string(),
            );
        }
        OperationFamily::Ffi => {
            repairs.push(
                "document the ABI, ownership, and lifetime contract for this FFI boundary"
                    .to_string(),
            );
            repairs.push(
                "attach sanitizer or cargo-careful receipt evidence after running the scoped command outside unsafe-review"
                    .to_string(),
            );
        }
        _ => {}
    }

    if missing_kind(card, "contract") {
        repairs.push("add or expose the local safety contract for this card".to_string());
    }
    if missing_kind(card, "test") {
        repairs
            .push("add or point to a focused test that exercises this owner or seam".to_string());
    }
    if missing_kind(card, "witness") {
        repairs.push(
            "attach a scoped witness receipt after running the suggested command outside unsafe-review"
                .to_string(),
        );
    }

    let has_card_scoped_repairs = !repairs.is_empty();
    if !has_card_scoped_repairs {
        repairs.push(card.next_action.summary.clone());
    }
    AllowedRepairs {
        repairs: dedupe_preserve_order(repairs),
        has_card_scoped_repairs,
    }
}

fn add_raw_pointer_repairs(card: &ReviewCard, repairs: &mut Vec<String>, alignment_required: bool) {
    if missing_discharge(card, "pointer-live") {
        repairs.push("add a same-pointer live/nullability guard before this operation".to_string());
    }
    if missing_discharge(card, "bounds") {
        repairs.push(
            "add a same-pointer or same-buffer bounds guard before this operation".to_string(),
        );
    }
    if alignment_required && missing_discharge(card, "alignment") {
        repairs.push(
            "add a same-pointer alignment guard, or switch to an unaligned operation only if unaligned input is intended"
                .to_string(),
        );
    }
    if missing_discharge(card, "initialized") {
        repairs.push(
            "show that memory is initialized for the accessed type before this operation"
                .to_string(),
        );
    }
    if missing_discharge(card, "allocation") {
        repairs.push(
            "show that the access stays inside one live allocation for this pointer".to_string(),
        );
    }
}

fn missing_discharge(card: &ReviewCard, key: &str) -> bool {
    card.obligation_evidence
        .iter()
        .any(|evidence| evidence.obligation.key == key && !evidence.discharge.present)
}

fn missing_kind(card: &ReviewCard, kind: &str) -> bool {
    card.missing.iter().any(|missing| missing.kind == kind)
}

fn dedupe_preserve_order(repairs: Vec<String>) -> Vec<String> {
    let mut deduped = Vec::new();
    for repair in repairs {
        if !deduped.contains(&repair) {
            deduped.push(repair);
        }
    }
    deduped
}

#[derive(Serialize)]
struct AgentCard<'a> {
    id: &'a str,
    #[serde(rename = "class")]
    class_name: &'static str,
    priority: &'static str,
    confidence: &'static str,
}

impl<'a> From<&'a ReviewCard> for AgentCard<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            id: &card.id.0,
            class_name: card.class.as_str(),
            priority: card.priority.as_str(),
            confidence: card.confidence.as_str(),
        }
    }
}

#[derive(Serialize)]
struct AgentContext<'a> {
    file: String,
    line: usize,
    column: usize,
    owner: &'a str,
    site_kind: &'static str,
    operation_family: &'static str,
    operation: &'a str,
    snippet: &'a str,
    hazards: Vec<&'static str>,
}

impl<'a> From<&'a ReviewCard> for AgentContext<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            file: path_display(&card.site.location.file),
            line: card.site.location.line,
            column: card.site.location.column,
            owner: card.site.owner.as_deref().unwrap_or(""),
            site_kind: card.site.kind.as_str(),
            operation_family: card.operation.family.as_str(),
            operation: &card.operation.expression,
            snippet: &card.site.snippet,
            hazards: card.hazards.iter().map(|hazard| hazard.as_str()).collect(),
        }
    }
}

#[derive(Serialize)]
struct AgentSafetyContract<'a> {
    required_conditions: Vec<&'a str>,
    contract_evidence: &'a str,
    discharge_evidence: &'a str,
    reach_evidence: &'a str,
    witness_evidence: &'a str,
    reach_limitation: &'static str,
}

impl<'a> From<&'a ReviewCard> for AgentSafetyContract<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            required_conditions: card
                .obligations
                .iter()
                .map(|obligation| obligation.description.as_str())
                .collect(),
            contract_evidence: &card.contract.summary,
            discharge_evidence: &card.discharge.summary,
            reach_evidence: &card.reach.summary,
            witness_evidence: &card.witness.summary,
            reach_limitation: "static reach evidence is not proof that the unsafe site executed",
        }
    }
}

#[derive(Serialize)]
struct AgentObligationEvidence<'a> {
    key: &'a str,
    description: &'a str,
    contract: AgentEvidenceState<'a>,
    discharge: AgentEvidenceState<'a>,
    reach: AgentEvidenceState<'a>,
    witness: AgentEvidenceState<'a>,
}

impl<'a> From<&'a ObligationEvidence> for AgentObligationEvidence<'a> {
    fn from(evidence: &'a ObligationEvidence) -> Self {
        Self {
            key: &evidence.obligation.key,
            description: &evidence.obligation.description,
            contract: AgentEvidenceState::from(&evidence.contract),
            discharge: AgentEvidenceState::from(&evidence.discharge),
            reach: AgentEvidenceState::from(&evidence.reach),
            witness: AgentEvidenceState::from(&evidence.witness),
        }
    }
}

#[derive(Serialize)]
struct AgentEvidenceState<'a> {
    present: bool,
    state: &'a str,
    summary: &'a str,
}

impl<'a> From<&'a EvidenceState> for AgentEvidenceState<'a> {
    fn from(state: &'a EvidenceState) -> Self {
        Self {
            present: state.present,
            state: &state.state,
            summary: &state.summary,
        }
    }
}

#[derive(Serialize)]
struct AgentMissingEvidence<'a> {
    kind: &'a str,
    message: &'a str,
}

#[derive(Serialize)]
struct AgentReadiness {
    ready: bool,
    state: &'static str,
    reasons: Vec<String>,
}

impl AgentReadiness {
    fn not_ready(state: &'static str, reasons: Vec<String>) -> Self {
        Self {
            ready: false,
            state,
            reasons,
        }
    }
}

#[derive(Serialize)]
struct AgentWitnessRoute<'a> {
    kind: &'static str,
    reason: &'a str,
    command: Option<&'a str>,
    required: bool,
}

impl<'a> From<&'a WitnessRoute> for AgentWitnessRoute<'a> {
    fn from(route: &'a WitnessRoute) -> Self {
        Self {
            kind: route.kind.as_str(),
            reason: &route.reason,
            command: route.command.as_deref(),
            required: route.required,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{
        AnalysisMode, AnalyzeInput, AnalyzeOutput, DiffSource, PolicyMode, Scope, analyze,
    };
    use std::path::PathBuf;

    #[test]
    fn agent_packet_is_parseable_bounded_and_card_sourced() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;

        assert_eq!(value["schema_version"], "0.1");
        assert_eq!(value["tool"], "unsafe-review");
        assert_eq!(value["mode"], "bounded_repair_packet");
        assert_eq!(value["source"], "review_card");
        assert_eq!(value["policy"], "advisory");
        assert_eq!(value["card_id"], card.id.0);
        assert_eq!(value["card"]["id"], card.id.0);
        assert_eq!(value["card"]["class"], "guard_missing");
        assert_eq!(
            value["context"]["operation"],
            "unsafe { ptr.cast::<Header>().read() }"
        );
        assert_eq!(value["context"]["operation_family"], "raw_pointer_read");
        assert!(value["safety_contract"]["required_conditions"].is_array());
        assert!(
            value["safety_contract"]["reach_limitation"]
                .as_str()
                .unwrap_or("")
                .contains("not proof")
        );
        assert!(value["required_safety_conditions"].is_array());
        assert!(value["obligation_evidence"].is_array());
        assert!(value["missing"].is_array());
        assert!(value["missing_evidence"].is_array());
        assert!(value["allowed_repairs"].is_array());
        assert_eq!(value["agent_readiness"]["ready"], true);
        assert_eq!(value["agent_readiness"]["state"], "ready");
        assert!(
            serde_json::to_string(&value["agent_readiness"]["reasons"])
                .map_err(|err| format!("render readiness reasons failed: {err}"))?
                .contains("card-scoped allowed repairs")
        );
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;
        assert!(allowed_repairs.contains("alignment guard"));
        assert!(allowed_repairs.contains("unaligned operation"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert_eq!(value["repair_scope"], "this card only");
        assert!(value["witness_routes"].is_array());
        assert!(value["verify_commands"].is_array());
        assert!(
            value["verify_commands"][0]
                .as_str()
                .unwrap_or("")
                .contains("cargo +nightly miri test read_header")
        );
        assert!(value["do_not_do"].is_array());
        assert!(
            serde_json::to_string(&value["do_not_do"])
                .map_err(|err| format!("render do_not_do failed: {err}"))?
                .contains("do not change unrelated unsafe code")
        );
        assert!(value["stop_conditions"].is_array());
        assert!(
            serde_json::to_string(&value["stop_conditions"])
                .map_err(|err| format!("render stop_conditions failed: {err}"))?
                .contains("same unsafe seam")
        );
        assert!(
            value["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not a Miri result")
        );
        Ok(())
    }

    #[test]
    fn agent_packet_scopes_copy_repairs_to_range_and_overlap() -> Result<(), String> {
        let output = fixture_output("copy_nonoverlapping")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;

        assert!(allowed_repairs.contains("source and destination ranges"));
        assert!(allowed_repairs.contains("do not overlap"));
        assert!(allowed_repairs.contains("count"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("alignment guard"));
        Ok(())
    }

    #[test]
    fn agent_packet_does_not_suggest_alignment_for_unaligned_read() -> Result<(), String> {
        let output = fixture_output("raw_pointer_read_unaligned")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;

        assert!(!allowed_repairs.contains("alignment guard"));
        assert!(!allowed_repairs.contains("unaligned operation"));
        assert!(allowed_repairs.contains("witness receipt"));
        Ok(())
    }

    #[test]
    fn agent_packet_routes_non_miri_cards_without_overclaiming() -> Result<(), String> {
        let output = fixture_output("ffi_sanitizer_route")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let routes = serde_json::to_string(&value["witness_routes"])
            .map_err(|err| format!("render routes failed: {err}"))?;

        assert!(routes.contains("asan"));
        assert!(routes.contains("cargo-careful"));
        assert!(!routes.contains("\"miri\""));
        assert_eq!(value["agent_readiness"]["ready"], false);
        assert_eq!(value["agent_readiness"]["state"], "needs_human_review");
        let reasons = serde_json::to_string(&value["agent_readiness"]["reasons"])
            .map_err(|err| format!("render readiness reasons failed: {err}"))?;
        assert!(reasons.contains("miri_unsupported"));
        assert!(reasons.contains("ffi"));
        assert!(
            value["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not UB-free status")
        );
        Ok(())
    }

    #[test]
    fn agent_packet_marks_inline_asm_as_not_ready_for_repair_delegation() -> Result<(), String> {
        let output = fixture_output("inline_asm_human_review")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;

        assert_eq!(value["context"]["operation_family"], "inline_asm");
        assert_eq!(value["agent_readiness"]["ready"], false);
        assert_eq!(value["agent_readiness"]["state"], "needs_human_review");
        let reasons = serde_json::to_string(&value["agent_readiness"]["reasons"])
            .map_err(|err| format!("render readiness reasons failed: {err}"))?;
        assert!(reasons.contains("inline_asm"));
        assert!(reasons.contains("human deep review"));
        Ok(())
    }

    fn fixture_output(name: &str) -> Result<AnalyzeOutput, String> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name);
        analyze(AnalyzeInput {
            root: root.clone(),
            scope: Scope::Diff,
            diff: DiffSource::File(root.join("change.diff")),
            mode: AnalysisMode::Draft,
            policy: PolicyMode::Advisory,
            include_unchanged_tests: true,
            max_cards: None,
        })
    }

    fn parse_json(text: &str) -> Result<serde_json::Value, String> {
        serde_json::from_str(text).map_err(|err| format!("JSON parse failed: {err}"))
    }
}
