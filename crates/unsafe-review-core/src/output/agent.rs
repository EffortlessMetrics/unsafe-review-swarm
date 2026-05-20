use crate::domain::{EvidenceState, ObligationEvidence, ReviewCard, WitnessRoute};
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
    allowed_repairs: Vec<&'a str>,
    repair_scope: &'static str,
    witness_routes: Vec<AgentWitnessRoute<'a>>,
    verify_commands: &'a [String],
    do_not_do: &'static [&'static str],
    stop_conditions: &'static [&'static str],
}

impl<'a> From<&'a ReviewCard> for AgentPacket<'a> {
    fn from(card: &'a ReviewCard) -> Self {
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
            allowed_repairs: vec![card.next_action.summary.as_str()],
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
    use crate::domain::{EvidenceState, ReviewCard};
    use serde_json::Value;
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
        assert_eq!(value["repair_scope"], "this card only");
        assert!(value["witness_routes"].is_array());
        assert!(value["verify_commands"].is_array());
        assert!(value["do_not_do"].is_array());
        assert!(
            serde_json::to_string(&value["do_not_do"])
                .map_err(|err| format!("render do_not_do failed: {err}"))?
                .contains("do not change unrelated unsafe code")
        );
        assert!(value["stop_conditions"].is_array());
        assert!(
            value["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not a Miri result")
        );
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
        assert!(
            value["trust_boundary"]
                .as_str()
                .unwrap_or("")
                .contains("not UB-free status")
        );
        Ok(())
    }

    #[test]
    fn agent_packet_mirrors_review_card_evidence() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let Some(card) = output.cards.first() else {
            return Err("fixture should emit one card".to_string());
        };
        let value = parse_json(&render(card))?;

        assert_eq!(json_str(&value["task"], "task")?, card.next_action.summary);
        assert_eq!(
            json_str(&value["context"]["operation"], "context.operation")?,
            card.operation.expression
        );
        assert_eq!(
            json_str(&value["context"]["snippet"], "context.snippet")?,
            card.site.snippet
        );
        assert_eq!(
            string_array(&value["context"]["hazards"], "context.hazards")?,
            card.hazards
                .iter()
                .map(|hazard| hazard.as_str().to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            string_array(
                &value["required_safety_conditions"],
                "required_safety_conditions"
            )?,
            card.obligations
                .iter()
                .map(|obligation| obligation.description.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            string_array(
                &value["safety_contract"]["required_conditions"],
                "safety_contract.required_conditions"
            )?,
            card.obligations
                .iter()
                .map(|obligation| obligation.description.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            json_str(
                &value["safety_contract"]["contract_evidence"],
                "safety_contract.contract_evidence"
            )?,
            card.contract.summary
        );
        assert_eq!(
            json_str(
                &value["safety_contract"]["discharge_evidence"],
                "safety_contract.discharge_evidence"
            )?,
            card.discharge.summary
        );
        assert_eq!(
            json_str(
                &value["safety_contract"]["reach_evidence"],
                "safety_contract.reach_evidence"
            )?,
            card.reach.summary
        );
        assert_eq!(
            json_str(
                &value["safety_contract"]["witness_evidence"],
                "safety_contract.witness_evidence"
            )?,
            card.witness.summary
        );
        assert_eq!(
            string_array(&value["missing"], "missing")?,
            card.missing
                .iter()
                .map(|missing| missing.message.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            string_array(&value["allowed_repairs"], "allowed_repairs")?,
            vec![card.next_action.summary.clone()]
        );
        assert_eq!(
            string_array(&value["verify_commands"], "verify_commands")?,
            card.next_action.verify_commands
        );
        assert_missing_evidence_matches_card(&value, card)?;
        assert_obligation_evidence_matches_card(&value, card)?;
        assert_witness_routes_match_card(&value, card)?;
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

    fn json_array<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>, String> {
        value
            .as_array()
            .ok_or_else(|| format!("`{path}` should be an array"))
    }

    fn json_str<'a>(value: &'a Value, path: &str) -> Result<&'a str, String> {
        value
            .as_str()
            .ok_or_else(|| format!("`{path}` should be a string"))
    }

    fn json_bool(value: &Value, path: &str) -> Result<bool, String> {
        value
            .as_bool()
            .ok_or_else(|| format!("`{path}` should be a boolean"))
    }

    fn string_array(value: &Value, path: &str) -> Result<Vec<String>, String> {
        json_array(value, path)?
            .iter()
            .enumerate()
            .map(|(index, item)| {
                json_str(item, &format!("{path}[{index}]")).map(ToString::to_string)
            })
            .collect()
    }

    fn assert_missing_evidence_matches_card(
        value: &Value,
        card: &ReviewCard,
    ) -> Result<(), String> {
        let missing = json_array(&value["missing_evidence"], "missing_evidence")?;
        assert_eq!(missing.len(), card.missing.len());
        for (index, expected) in card.missing.iter().enumerate() {
            let actual = &missing[index];
            assert_eq!(
                json_str(&actual["kind"], &format!("missing_evidence[{index}].kind"))?,
                expected.kind
            );
            assert_eq!(
                json_str(
                    &actual["message"],
                    &format!("missing_evidence[{index}].message")
                )?,
                expected.message
            );
        }
        Ok(())
    }

    fn assert_obligation_evidence_matches_card(
        value: &Value,
        card: &ReviewCard,
    ) -> Result<(), String> {
        let obligations = json_array(&value["obligation_evidence"], "obligation_evidence")?;
        assert_eq!(obligations.len(), card.obligation_evidence.len());
        for (index, expected) in card.obligation_evidence.iter().enumerate() {
            let actual = &obligations[index];
            assert_eq!(
                json_str(&actual["key"], &format!("obligation_evidence[{index}].key"))?,
                expected.obligation.key
            );
            assert_eq!(
                json_str(
                    &actual["description"],
                    &format!("obligation_evidence[{index}].description")
                )?,
                expected.obligation.description
            );
            assert_evidence_state(
                &actual["contract"],
                &expected.contract,
                &format!("obligation_evidence[{index}].contract"),
            )?;
            assert_evidence_state(
                &actual["discharge"],
                &expected.discharge,
                &format!("obligation_evidence[{index}].discharge"),
            )?;
            assert_evidence_state(
                &actual["reach"],
                &expected.reach,
                &format!("obligation_evidence[{index}].reach"),
            )?;
            assert_evidence_state(
                &actual["witness"],
                &expected.witness,
                &format!("obligation_evidence[{index}].witness"),
            )?;
        }
        Ok(())
    }

    fn assert_evidence_state(
        actual: &Value,
        expected: &EvidenceState,
        path: &str,
    ) -> Result<(), String> {
        assert_eq!(
            json_bool(&actual["present"], &format!("{path}.present"))?,
            expected.present
        );
        assert_eq!(
            json_str(&actual["state"], &format!("{path}.state"))?,
            expected.state
        );
        assert_eq!(
            json_str(&actual["summary"], &format!("{path}.summary"))?,
            expected.summary
        );
        Ok(())
    }

    fn assert_witness_routes_match_card(value: &Value, card: &ReviewCard) -> Result<(), String> {
        let routes = json_array(&value["witness_routes"], "witness_routes")?;
        assert_eq!(routes.len(), card.routes.len());
        for (index, expected) in card.routes.iter().enumerate() {
            let actual = &routes[index];
            assert_eq!(
                json_str(&actual["kind"], &format!("witness_routes[{index}].kind"))?,
                expected.kind.as_str()
            );
            assert_eq!(
                json_str(
                    &actual["reason"],
                    &format!("witness_routes[{index}].reason")
                )?,
                expected.reason
            );
            match expected.command.as_deref() {
                Some(command) => assert_eq!(
                    json_str(
                        &actual["command"],
                        &format!("witness_routes[{index}].command")
                    )?,
                    command
                ),
                None => assert!(
                    actual["command"].is_null(),
                    "witness_routes[{index}].command should be null"
                ),
            }
            assert_eq!(
                json_bool(
                    &actual["required"],
                    &format!("witness_routes[{index}].required")
                )?,
                expected.required
            );
        }
        Ok(())
    }
}
