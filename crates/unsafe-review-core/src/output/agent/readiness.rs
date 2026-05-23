use super::AgentReadiness;
use crate::domain::{Confidence, OperationFamily, ReviewCard, ReviewClass, WitnessKind};

pub(super) fn build(card: &ReviewCard, has_card_scoped_repairs: bool) -> AgentReadiness {
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
