use crate::domain::ReviewCard;
use crate::output::REVIEWCARD_TRUST_BOUNDARY;
use serde::Serialize;

#[derive(Clone, Serialize)]
pub(crate) struct BuildThisFirstCue {
    kind: &'static str,
    command: Option<String>,
    route_kind: Option<&'static str>,
    pub(crate) summary: String,
}

impl BuildThisFirstCue {
    fn new(
        kind: &'static str,
        command: Option<String>,
        route_kind: Option<&'static str>,
        summary: String,
    ) -> Self {
        Self {
            kind,
            command,
            route_kind,
            summary,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct ConfirmationCue {
    hypothesis_to_confirm: String,
    build_this_first: BuildThisFirstCue,
    confirmation_step: String,
    trust_boundary: &'static str,
}

impl From<&ReviewCard> for ConfirmationCue {
    fn from(card: &ReviewCard) -> Self {
        Self {
            hypothesis_to_confirm: hypothesis_to_confirm(card),
            build_this_first: build_this_first(card),
            confirmation_step: confirmation_step(card),
            trust_boundary: REVIEWCARD_TRUST_BOUNDARY,
        }
    }
}

pub(crate) fn hypothesis_to_confirm(card: &ReviewCard) -> String {
    format!(
        "static `{}` ReviewCard for `{}`; confirm with external evidence before treating it as observed runtime behavior",
        card.class.as_str(),
        one_line(&card.operation.expression)
    )
}

pub(crate) fn build_this_first(card: &ReviewCard) -> BuildThisFirstCue {
    if let Some(command) = card.next_action.verify_commands.first() {
        return BuildThisFirstCue::new(
            "verify_command",
            Some(command.clone()),
            card.routes.first().map(|route| route.kind.as_str()),
            format!(
                "Build/run `{command}` first for this card; attach a matching receipt only if it confirms the route"
            ),
        );
    }
    if let Some(route) = card.routes.first() {
        return BuildThisFirstCue::new(
            "witness_route",
            route.command.clone(),
            Some(route.kind.as_str()),
            format!(
                "No automatic build/run command is available; use the `{}` route in `witness-plan.md` to derive a focused repro or human review before upgrading confidence",
                route.kind.as_str()
            ),
        );
    }
    BuildThisFirstCue::new(
        "human_review",
        None,
        None,
        "No automatic build/run command is available; derive the first confirmation from `unsafe-review explain` and human review before upgrading confidence".to_string(),
    )
}

pub(crate) fn confirmation_step(card: &ReviewCard) -> String {
    if let Some(command) = card.next_action.verify_commands.first() {
        return format!(
            "build/run `{command}` first, then attach a matching receipt if it confirms the route"
        );
    }
    if let Some(route) = card.routes.first() {
        return format!(
            "use the `{}` route in `witness-plan.md` to derive a focused repro or human review before upgrading confidence",
            route.kind.as_str()
        );
    }
    "derive a focused confirmation from `unsafe-review explain` and human review before upgrading confidence".to_string()
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
