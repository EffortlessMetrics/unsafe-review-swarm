use super::queue::{AgentReadiness, REQUIRES_HUMAN_REVIEW, REQUIRES_WITNESS_RECEIPT, UNSUPPORTED};
use crate::domain::{Confidence, OperationFamily, ReviewCard, ReviewClass, WitnessKind};

pub(super) fn build(card: &ReviewCard, has_card_scoped_repairs: bool) -> AgentReadiness {
    let mut reasons = Vec::new();
    if !card.class.is_actionable() {
        reasons.push(format!(
            "card class `{}` is not an open actionable repair target",
            card.class.as_str()
        ));
        return AgentReadiness::not_ready(UNSUPPORTED, reasons);
    }
    if card.missing.is_empty() {
        reasons.push("card has no missing evidence to repair".to_string());
        return AgentReadiness::not_ready(UNSUPPORTED, reasons);
    }
    if card.missing.iter().all(|missing| missing.kind == "witness") {
        reasons.push(
            "remaining work is an external witness receipt, not an automatic source repair"
                .to_string(),
        );
        return AgentReadiness::not_ready(REQUIRES_WITNESS_RECEIPT, reasons);
    }
    let mut requires_human_review = false;
    let mut requires_witness_receipt = false;
    if matches!(
        card.class,
        ReviewClass::RequiresSanitizer
            | ReviewClass::RequiresKaniOrCrux
            | ReviewClass::RequiresLoom
    ) {
        reasons.push(format!(
            "card class `{}` requires an external witness receipt before repair delegation",
            card.class.as_str()
        ));
        requires_witness_receipt = true;
    }
    if matches!(
        card.class,
        ReviewClass::StaticUnknown | ReviewClass::MiriUnsupported
    ) {
        reasons.push(format!(
            "card class `{}` requires human review before repair delegation",
            card.class.as_str()
        ));
        requires_human_review = true;
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
        requires_human_review = true;
    }
    if card
        .routes
        .iter()
        .any(|route| matches!(route.kind, WitnessKind::HumanDeepReview))
    {
        reasons.push("witness route requires human deep review".to_string());
        requires_human_review = true;
    }
    if card
        .routes
        .iter()
        .any(|route| matches!(route.kind, WitnessKind::Unsupported))
    {
        reasons.push("witness route is unsupported for bounded agent repair".to_string());
        return AgentReadiness::not_ready(UNSUPPORTED, reasons);
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
        AgentReadiness::ready_for_agent(vec![
            "specific operation family".to_string(),
            "card-scoped allowed repairs".to_string(),
            "verify commands available".to_string(),
            "medium-or-high confidence".to_string(),
        ])
    } else if requires_human_review {
        AgentReadiness::not_ready(REQUIRES_HUMAN_REVIEW, reasons)
    } else if requires_witness_receipt {
        AgentReadiness::not_ready(REQUIRES_WITNESS_RECEIPT, reasons)
    } else {
        AgentReadiness::not_ready(UNSUPPORTED, reasons)
    }
}
