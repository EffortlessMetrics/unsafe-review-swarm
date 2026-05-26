use crate::domain::{EvidenceState, ObligationEvidence, RelatedTest, ReviewCard, WitnessRoute};
use crate::util::path_display;
use serde::Serialize;

const TRUST_BOUNDARY: &str = "Static unsafe contract review only; this is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.";
const MAX_CONTEXT_EVIDENCE: usize = 3;
const MAX_RELATED_TESTS: usize = 3;

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
    source_context: AgentSourceContext<'a>,
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
            source_context: AgentSourceContext::from(card),
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
    readiness::build(card, has_card_scoped_repairs)
}

fn allowed_repairs(card: &ReviewCard) -> AllowedRepairs {
    repairs::build(card)
}

struct AllowedRepairs {
    repairs: Vec<String>,
    has_card_scoped_repairs: bool,
}

mod readiness;
mod repairs;

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
struct AgentSourceContext<'a> {
    unsafe_site: AgentSourceSite<'a>,
    nearby_safety_contract: Option<AgentContextEvidence<'a>>,
    nearby_guard_evidence: Vec<AgentContextEvidence<'a>>,
    related_tests: Vec<AgentRelatedTest<'a>>,
    limits: &'static [&'static str],
}

impl<'a> From<&'a ReviewCard> for AgentSourceContext<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        let nearby_safety_contract = card.contract.present.then_some(AgentContextEvidence {
            kind: "safety_contract",
            key: None,
            summary: &card.contract.summary,
        });
        let nearby_guard_evidence = card
            .obligation_evidence
            .iter()
            .filter(|evidence| evidence.discharge.present)
            .take(MAX_CONTEXT_EVIDENCE)
            .map(|evidence| AgentContextEvidence {
                kind: "guard_evidence",
                key: Some(evidence.obligation.key.as_str()),
                summary: &evidence.discharge.summary,
            })
            .collect();
        let related_tests = card
            .related_tests
            .iter()
            .take(MAX_RELATED_TESTS)
            .map(AgentRelatedTest::from)
            .collect();

        Self {
            unsafe_site: AgentSourceSite::from(card),
            nearby_safety_contract,
            nearby_guard_evidence,
            related_tests,
            limits: &[
                "bounded source context only; this packet does not include whole files",
                "related test mentions do not prove the unsafe site executed",
                "evidence summaries are ReviewCard projections, not independent analyzer truth",
            ],
        }
    }
}

#[derive(Serialize)]
struct AgentSourceSite<'a> {
    file: String,
    line: usize,
    column: usize,
    owner: &'a str,
    snippet: &'a str,
}

impl<'a> From<&'a ReviewCard> for AgentSourceSite<'a> {
    fn from(card: &'a ReviewCard) -> Self {
        Self {
            file: path_display(&card.site.location.file),
            line: card.site.location.line,
            column: card.site.location.column,
            owner: card.site.owner.as_deref().unwrap_or(""),
            snippet: &card.site.snippet,
        }
    }
}

#[derive(Serialize)]
struct AgentContextEvidence<'a> {
    kind: &'static str,
    key: Option<&'a str>,
    summary: &'a str,
}

#[derive(Serialize)]
struct AgentRelatedTest<'a> {
    name: &'a str,
    file: &'a str,
    line: usize,
}

impl<'a> From<&'a RelatedTest> for AgentRelatedTest<'a> {
    fn from(test: &'a RelatedTest) -> Self {
        Self {
            name: &test.name,
            file: &test.file,
            line: test.line,
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
        assert_eq!(value["source_context"]["unsafe_site"]["file"], "src/lib.rs");
        assert_eq!(
            value["source_context"]["unsafe_site"]["snippet"],
            "unsafe { ptr.cast::<Header>().read() }"
        );
        assert!(
            value["source_context"]["nearby_safety_contract"]["summary"]
                .as_str()
                .unwrap_or("")
                .contains("SAFETY")
        );
        assert_eq!(
            value["source_context"]["nearby_guard_evidence"][0]["key"],
            "bounds"
        );
        assert!(
            value["source_context"]["nearby_guard_evidence"][0]["summary"]
                .as_str()
                .unwrap_or("")
                .contains("bounds guard")
        );
        assert_eq!(
            value["source_context"]["related_tests"][0]["name"],
            "read_header"
        );
        assert!(
            serde_json::to_string(&value["source_context"]["limits"])
                .map_err(|err| format!("render source context limits failed: {err}"))?
                .contains("does not include whole files")
        );
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
    fn agent_packet_scopes_get_unchecked_repairs_to_same_slice_and_index() -> Result<(), String> {
        let output = fixture_output("get_unchecked_mut_bounds")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;

        assert_eq!(value["context"]["operation_family"], "get_unchecked");
        assert!(allowed_repairs.contains("same-slice length/range guard"));
        assert!(allowed_repairs.contains("same index value"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("alignment guard"));
        assert_eq!(value["agent_readiness"]["ready"], true);
        assert_eq!(value["agent_readiness"]["state"], "ready");
        Ok(())
    }

    #[test]
    fn agent_packet_scopes_maybeuninit_repairs_to_same_slot_initialization() -> Result<(), String> {
        let output = fixture_output("maybeuninit_assume_init")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;

        assert_eq!(
            value["context"]["operation_family"],
            "maybe_uninit_assume_init"
        );
        assert!(allowed_repairs.contains("same `MaybeUninit` slot"));
        assert!(allowed_repairs.contains("initialization branch open"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("alignment guard"));
        assert!(!allowed_repairs.contains("same-slice"));
        assert_eq!(value["agent_readiness"]["ready"], true);
        assert_eq!(value["agent_readiness"]["state"], "ready");
        Ok(())
    }

    #[test]
    fn agent_packet_scopes_transmute_repairs_to_layout_and_valid_value() -> Result<(), String> {
        let output = fixture_output("transmute_invalid_value")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;

        assert_eq!(value["context"]["operation_family"], "transmute");
        assert!(allowed_repairs.contains("source and destination layouts"));
        assert!(allowed_repairs.contains("valid-value domain"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("same-slice"));
        assert!(!allowed_repairs.contains("same `MaybeUninit` slot"));
        assert_eq!(value["agent_readiness"]["ready"], true);
        assert_eq!(value["agent_readiness"]["state"], "ready");
        Ok(())
    }

    #[test]
    fn agent_packet_scopes_unwrap_unchecked_repairs_to_same_receiver_state() -> Result<(), String> {
        let output = fixture_output("unwrap_unchecked_result")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;

        assert_eq!(value["context"]["operation_family"], "unwrap_unchecked");
        assert!(allowed_repairs.contains("same-receiver"));
        assert!(allowed_repairs.contains("`Some` or `Ok` guard"));
        assert!(allowed_repairs.contains("same receiver value"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("same-slice"));
        assert!(!allowed_repairs.contains("valid-value domain"));
        assert_eq!(value["agent_readiness"]["ready"], true);
        assert_eq!(value["agent_readiness"]["state"], "ready");
        Ok(())
    }

    #[test]
    fn agent_packet_scopes_unreachable_unchecked_repairs_to_same_control_path() -> Result<(), String>
    {
        let output = fixture_output("unreachable_unchecked_path")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;

        assert_eq!(
            value["context"]["operation_family"],
            "unreachable_unchecked"
        );
        assert!(allowed_repairs.contains("same control-flow path"));
        assert!(allowed_repairs.contains("safe return, error, or panic path"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("same-receiver"));
        assert!(!allowed_repairs.contains("valid-value domain"));
        assert_eq!(value["agent_readiness"]["ready"], true);
        assert_eq!(value["agent_readiness"]["state"], "ready");
        Ok(())
    }

    #[test]
    fn agent_packet_scopes_zeroed_repairs_to_valid_zero_target_type() -> Result<(), String> {
        let output = fixture_output("zeroed_invalid_value")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;

        assert_eq!(value["context"]["operation_family"], "zeroed");
        assert!(allowed_repairs.contains("all-zero bit pattern"));
        assert!(allowed_repairs.contains("this target type"));
        assert!(allowed_repairs.contains("explicit constructor"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("same control-flow path"));
        assert!(!allowed_repairs.contains("same-receiver"));
        assert_eq!(value["agent_readiness"]["ready"], true);
        assert_eq!(value["agent_readiness"]["state"], "ready");
        Ok(())
    }

    #[test]
    fn agent_packet_scopes_box_from_raw_repairs_to_same_pointer_ownership() -> Result<(), String> {
        let output = fixture_output("box_from_raw")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;

        assert_eq!(value["context"]["operation_family"], "box_from_raw");
        assert!(allowed_repairs.contains("same raw pointer"));
        assert!(allowed_repairs.contains("Box::into_raw"));
        assert!(allowed_repairs.contains("compatible allocator"));
        assert!(allowed_repairs.contains("unique ownership"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("same-slice"));
        assert!(!allowed_repairs.contains("all-zero bit pattern"));
        assert_eq!(value["agent_readiness"]["ready"], true);
        assert_eq!(value["agent_readiness"]["state"], "ready");
        Ok(())
    }

    #[test]
    fn agent_packet_scopes_drop_in_place_repairs_to_drop_obligations() -> Result<(), String> {
        let output = fixture_output("drop_in_place_deallocation")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;

        assert_eq!(value["context"]["operation_family"], "drop_in_place");
        assert!(allowed_repairs.contains("pointed-to value is initialized"));
        assert!(allowed_repairs.contains("ownership of the pointee"));
        assert!(allowed_repairs.contains("dropped again"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("Box::from_raw"));
        assert!(!allowed_repairs.contains("all-zero bit pattern"));
        assert_eq!(value["agent_readiness"]["ready"], true);
        assert_eq!(value["agent_readiness"]["state"], "ready");
        Ok(())
    }

    #[test]
    fn agent_packet_scopes_pin_unchecked_repairs_to_pin_invariant() -> Result<(), String> {
        let output = fixture_output("pin_new_unchecked")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;
        let reasons = serde_json::to_string(&value["agent_readiness"]["reasons"])
            .map_err(|err| format!("render readiness reasons failed: {err}"))?;
        let routes = serde_json::to_string(&value["witness_routes"])
            .map_err(|err| format!("render routes failed: {err}"))?;

        assert_eq!(value["context"]["operation_family"], "pin_unchecked");
        assert!(allowed_repairs.contains("will not move"));
        assert!(allowed_repairs.contains("pinning invariant"));
        assert!(allowed_repairs.contains("safe `Pin::new`"));
        assert!(allowed_repairs.contains("pinned-owner"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("same raw pointer"));
        assert!(!allowed_repairs.contains("all-zero bit pattern"));
        assert!(!allowed_repairs.contains("same control-flow path"));
        assert_eq!(value["agent_readiness"]["ready"], false);
        assert_eq!(value["agent_readiness"]["state"], "needs_human_review");
        assert!(reasons.contains("human deep review"));
        assert!(reasons.contains("no verify command"));
        assert!(routes.contains("human-deep-review"));
        Ok(())
    }

    #[test]
    fn agent_packet_scopes_target_feature_repairs_to_dispatch_invariant() -> Result<(), String> {
        let output = fixture_output("target_feature_missing_safety_docs")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;
        let reasons = serde_json::to_string(&value["agent_readiness"]["reasons"])
            .map_err(|err| format!("render readiness reasons failed: {err}"))?;
        let routes = serde_json::to_string(&value["witness_routes"])
            .map_err(|err| format!("render routes failed: {err}"))?;

        assert_eq!(value["context"]["operation_family"], "target_feature");
        assert!(allowed_repairs.contains("matching runtime or compile-time feature check"));
        assert!(allowed_repairs.contains("non-`target_feature` fallback"));
        assert!(allowed_repairs.contains("cfg/feature gating"));
        assert!(allowed_repairs.contains("local safety contract"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("same raw pointer"));
        assert!(!allowed_repairs.contains("all-zero bit pattern"));
        assert!(!allowed_repairs.contains("will not move"));
        assert_eq!(value["agent_readiness"]["ready"], false);
        assert_eq!(value["agent_readiness"]["state"], "needs_human_review");
        assert!(reasons.contains("target_feature"));
        assert!(reasons.contains("human deep review"));
        assert!(reasons.contains("no verify command"));
        assert!(routes.contains("human-deep-review"));
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
    fn agent_packet_marks_loom_routed_cards_as_not_ready_for_repair_delegation()
    -> Result<(), String> {
        let output = fixture_output("atomic_pointer_state_fetch_ops")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit at least one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let routes = serde_json::to_string(&value["witness_routes"])
            .map_err(|err| format!("render routes failed: {err}"))?;

        assert_eq!(value["context"]["operation_family"], "atomic_pointer_state");
        assert_eq!(value["card"]["class"], "requires_loom");
        assert!(routes.contains("loom"));
        assert!(routes.contains("shuttle"));
        assert!(!routes.contains("\"miri\""));
        assert_eq!(value["agent_readiness"]["ready"], false);
        assert_eq!(value["agent_readiness"]["state"], "needs_human_review");
        let reasons = serde_json::to_string(&value["agent_readiness"]["reasons"])
            .map_err(|err| format!("render readiness reasons failed: {err}"))?;
        assert!(reasons.contains("requires_loom"));
        assert!(reasons.contains("external witness routing"));
        Ok(())
    }

    #[test]
    fn agent_packet_scopes_static_mut_repairs_to_global_state_invariant() -> Result<(), String> {
        let output = fixture_output("static_mut_global_state")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;
        let allowed_repairs = serde_json::to_string(&value["allowed_repairs"])
            .map_err(|err| format!("render allowed repairs failed: {err}"))?;
        let reasons = serde_json::to_string(&value["agent_readiness"]["reasons"])
            .map_err(|err| format!("render readiness reasons failed: {err}"))?;
        let routes = serde_json::to_string(&value["witness_routes"])
            .map_err(|err| format!("render routes failed: {err}"))?;

        assert_eq!(value["context"]["operation_family"], "static_mut");
        assert_eq!(value["card"]["class"], "requires_loom");
        assert!(allowed_repairs.contains("synchronized"));
        assert!(allowed_repairs.contains("one execution context"));
        assert!(allowed_repairs.contains("aliased mutable references"));
        assert!(allowed_repairs.contains("data races"));
        assert!(allowed_repairs.contains("UnsafeCell"));
        assert!(allowed_repairs.contains("witness receipt"));
        assert!(!allowed_repairs.contains("same raw pointer"));
        assert!(!allowed_repairs.contains("all-zero bit pattern"));
        assert!(!allowed_repairs.contains("target_feature"));
        assert_eq!(value["agent_readiness"]["ready"], false);
        assert_eq!(value["agent_readiness"]["state"], "needs_human_review");
        assert!(reasons.contains("requires_loom"));
        assert!(reasons.contains("external witness routing"));
        assert!(routes.contains("loom"));
        assert!(routes.contains("shuttle"));
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
