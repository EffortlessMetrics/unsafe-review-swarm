use crate::domain::ReviewCard;
use crate::output::REVIEWCARD_TRUST_BOUNDARY;
use crate::util::path_display;
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
    minimal_repro: MinimalReproCue,
    confirmation_step: String,
    /// True only when an imported witness receipt records that a runtime
    /// witness tool actually executed for this card. It reflects the imported
    /// receipt's claim; unsafe-review did not run anything itself.
    runtime_executed: bool,
    /// Derived per-card confirmation state
    /// (pending/receipt_imported/executed/confirmed/not_reproduced/
    /// inconclusive). "not_reproduced" is a single-run observation, not a
    /// safety claim. See `WitnessEvidence::confirmation_state`.
    confirmation_state: &'static str,
    trust_boundary: &'static str,
}

impl From<&ReviewCard> for ConfirmationCue {
    fn from(card: &ReviewCard) -> Self {
        Self {
            hypothesis_to_confirm: hypothesis_to_confirm(card),
            build_this_first: build_this_first(card),
            minimal_repro: minimal_repro(card),
            confirmation_step: confirmation_step(card),
            runtime_executed: card.witness.runtime_executed,
            confirmation_state: card.witness.confirmation_state(),
            trust_boundary: REVIEWCARD_TRUST_BOUNDARY,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct MinimalReproCue {
    kind: &'static str,
    command: Option<String>,
    route_kind: Option<&'static str>,
    steps: Vec<String>,
    limitation: &'static str,
}

impl MinimalReproCue {
    fn new(
        kind: &'static str,
        command: Option<String>,
        route_kind: Option<&'static str>,
        steps: Vec<String>,
    ) -> Self {
        Self {
            kind,
            command,
            route_kind,
            steps,
            limitation: "Minimal repro cue only; unsafe-review did not run this command, observe runtime behavior, prove site execution, prove UB, or prove repository safety.",
        }
    }

    pub(crate) fn steps(&self) -> &[String] {
        &self.steps
    }

    pub(crate) fn limitation(&self) -> &'static str {
        self.limitation
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

pub(crate) fn minimal_repro(card: &ReviewCard) -> MinimalReproCue {
    let identity_step = format!(
        "Confirm ReviewCard `{}` still maps to `{}` at `{}` before upgrading confidence.",
        card.id,
        one_line(&card.operation.expression),
        location_label(card)
    );
    if let Some(command) = card.next_action.verify_commands.first() {
        return MinimalReproCue::new(
            "verify_command",
            Some(command.clone()),
            card.routes.first().map(|route| route.kind.as_str()),
            vec![
                identity_step,
                format!("Build/run `{command}` as the smallest available command for this card."),
                "Attach a matching receipt only if that run confirms the same route and ReviewCard identity.".to_string(),
            ],
        );
    }
    if let Some(route) = card.routes.first() {
        let route_step = if let Some(command) = &route.command {
            format!(
                "Use the `{}` route from `witness-plan.md`; start with `{command}` if it still targets this card.",
                route.kind.as_str()
            )
        } else {
            format!(
                "Use the `{}` route from `witness-plan.md` to derive a focused repro or human review target for this card.",
                route.kind.as_str()
            )
        };
        return MinimalReproCue::new(
            "witness_route",
            route.command.clone(),
            Some(route.kind.as_str()),
            vec![
                identity_step,
                route_step,
                "Attach a matching receipt only if the route confirms this card; otherwise keep the finding as a hypothesis to review.".to_string(),
            ],
        );
    }
    MinimalReproCue::new(
        "human_review",
        None,
        None,
        vec![
            identity_step,
            format!(
                "Use `unsafe-review explain {}` and human review to derive a focused repro before upgrading confidence.",
                card.id
            ),
            "Keep the finding advisory unless external evidence confirms the same route."
                .to_string(),
        ],
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

fn location_label(card: &ReviewCard) -> String {
    format!(
        "{}:{}:{}",
        path_display(&card.site.location.file),
        card.site.location.line,
        card.site.location.column
    )
}
