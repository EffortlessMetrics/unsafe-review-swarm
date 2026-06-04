use crate::api::AnalyzeOutput;
use crate::domain::{ReviewCard, WitnessKind, WitnessRoute};
use crate::output::{NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE};
use crate::util::path_display;

struct RouteGroup {
    heading: &'static str,
    kinds: &'static [WitnessKind],
    include_unrouted: bool,
    limit: &'static str,
}

const ROUTE_GROUPS: &[RouteGroup] = &[
    RouteGroup {
        heading: "Miri / cargo-careful",
        kinds: &[WitnessKind::Miri, WitnessKind::CargoCareful],
        include_unrouted: false,
        limit: "Concrete runtime evidence is path-specific. It can support the exercised route, but it does not prove arbitrary callers, repo safety, UB-free status, or site execution unless a matching receipt records the run.",
    },
    RouteGroup {
        heading: "Sanitizers",
        kinds: &[
            WitnessKind::AddressSanitizer,
            WitnessKind::MemorySanitizer,
            WitnessKind::ThreadSanitizer,
            WitnessKind::LeakSanitizer,
        ],
        include_unrouted: false,
        limit: "Sanitizers are configured runtime diagnostics for exercised inputs and builds. They do not prove every input, platform, aliasing case, or foreign boundary safe.",
    },
    RouteGroup {
        heading: "Loom / Shuttle",
        kinds: &[WitnessKind::Loom, WitnessKind::Shuttle],
        include_unrouted: false,
        limit: "Concurrency witnesses explore modeled scheduler interleavings. They do not prove behavior outside the modeled harness, assumptions, or state space.",
    },
    RouteGroup {
        heading: "Kani / Crux",
        kinds: &[WitnessKind::Kani, WitnessKind::Crux],
        include_unrouted: false,
        limit: "Proof harnesses are scoped to encoded preconditions, bounds, and assertions. They do not prove broader callers or undocumented contracts.",
    },
    RouteGroup {
        heading: "Human deep review / unsupported",
        kinds: &[WitnessKind::HumanDeepReview, WitnessKind::Unsupported],
        include_unrouted: true,
        limit: "Manual review is the route when local static evidence and known witness adapters are not enough. Record assumptions and limits instead of converting uncertainty into a proof claim.",
    },
];

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    let mut out = String::new();
    out.push_str("# unsafe-review witness plan\n\n");
    out.push_str(&format!("- Review cards: {}\n", output.summary.cards));
    out.push_str(&format!(
        "- Open actionable gaps: {}\n",
        output.summary.open_actionable_gaps
    ));
    out.push_str(&format!("- Policy mode: `{}`\n\n", output.policy.as_str()));

    if output.cards.is_empty() {
        out.push_str(NO_CHANGED_GAPS_MESSAGE);
        out.push('\n');
        out.push_str(NO_CHANGED_GAPS_LIMITATION);
        out.push_str(
            "\n\nNo witness routes are recommended because no review cards were emitted.\n\n",
        );
    } else {
        out.push_str("## Route groups\n\n");
        for group in ROUTE_GROUPS {
            render_group(&mut out, group, &output.cards);
        }
    }

    out.push_str("## Trust boundary\n\n");
    out.push_str("This artifact is static unsafe contract review. It routes reviewers to credible witnesses but does not run Miri, cargo-careful, sanitizers, Loom, Shuttle, Kani, or Crux. It is not a proof of memory safety, not UB-free status, and not a Miri result unless a witness receipt is attached.\n");
    out
}

fn render_group(out: &mut String, group: &RouteGroup, cards: &[ReviewCard]) {
    let matching = cards
        .iter()
        .filter_map(|card| {
            let routes = routes_for_group(card, group);
            if routes.is_empty() && !(group.include_unrouted && card.routes.is_empty()) {
                None
            } else {
                Some((card, routes))
            }
        })
        .collect::<Vec<_>>();

    if matching.is_empty() {
        return;
    }

    out.push_str(&format!("### {}\n\n", group.heading));
    out.push_str(&format!("- Limit: {}\n\n", group.limit));

    for (card, routes) in matching {
        render_group_card(out, card, &routes);
    }
}

fn routes_for_group<'a>(card: &'a ReviewCard, group: &RouteGroup) -> Vec<&'a WitnessRoute> {
    card.routes
        .iter()
        .filter(|route| group.kinds.contains(&route.kind))
        .collect()
}

fn render_group_card(out: &mut String, card: &ReviewCard, routes: &[&WitnessRoute]) {
    out.push_str(&format!(
        "#### `{}`\n\n- Class: `{}`\n- Proof path: `{}`\n- Location: {}:{}\n- Operation family: `{}`\n- Operation: `{}`\n- Hazards: {}\n- Missing evidence: {}\n- Witness evidence: {}\n",
        card.id,
        card.class.as_str(),
        card.proof_path.as_str(),
        path_display(&card.site.location.file),
        card.site.location.line,
        card.operation.family.as_str(),
        one_line(&card.operation.expression),
        hazard_summary(card),
        missing_summary(card),
        card.witness.summary
    ));
    render_obligation_evidence(out, card);
    out.push_str(&format!(
        "- Next action: {}\n",
        one_line(&card.next_action.summary)
    ));
    out.push_str(&format!(
        "- Hypothesis to confirm: {}\n",
        card_hypothesis(card)
    ));
    out.push_str(&format!(
        "- Confirmation step: {}\n",
        card_confirmation_step(card)
    ));
    if !card.next_action.verify_commands.is_empty() {
        out.push_str("- Verify command");
        if card.next_action.verify_commands.len() > 1 {
            out.push('s');
        }
        out.push_str(":\n\n");
        for command in &card.next_action.verify_commands {
            out.push_str("```bash\n");
            out.push_str(command);
            out.push_str("\n```\n");
        }
    }

    if routes.is_empty() {
        out.push_str("- Route: `human-deep-review`\n");
        out.push_str("  - Reason: no automatic witness route was selected\n\n");
        out.push_str("  - What it can show: reviewer-owned contract reasoning, assumptions, and missing context.\n");
        out.push_str("  - What it cannot prove: memory safety, UB-free status, site execution, or witness success.\n");
        out.push_str("  - Receipt hint: no saved-output receipt applies until an external witness run exists.\n\n");
        return;
    }

    for route in routes {
        out.push_str(&format!(
            "- Route: `{}`{}\n",
            route.kind.as_str(),
            if route.required { " (required)" } else { "" }
        ));
        out.push_str(&format!("  - Reason: {}\n", route.reason));
        out.push_str(&format!(
            "  - What it can show: {}\n",
            route_can_show(route.kind)
        ));
        out.push_str(&format!(
            "  - What it cannot prove: {}\n",
            route_cannot_prove(route.kind)
        ));
        if let Some(command) = &route.command {
            out.push_str("  - Command:\n\n");
            out.push_str("```bash\n");
            out.push_str(command);
            out.push_str("\n```\n");
        } else {
            out.push_str("  - Command: no automatic command; route to human review.\n");
        }
        out.push_str(&format!(
            "  - Receipt hint: {}\n",
            receipt_hint(card, route)
        ));
    }
    out.push('\n');
}

fn render_obligation_evidence(out: &mut String, card: &ReviewCard) {
    out.push_str("- Required safety conditions:\n");
    if card.obligation_evidence.is_empty() {
        out.push_str("  - none recorded\n");
    } else {
        for evidence in &card.obligation_evidence {
            out.push_str(&format!(
                "  - `{}`: {}\n",
                evidence.obligation.key,
                one_line(&evidence.obligation.description)
            ));
        }
    }

    out.push_str("- Obligation evidence:\n");
    if card.obligation_evidence.is_empty() {
        out.push_str("  - none recorded\n");
    } else {
        for evidence in &card.obligation_evidence {
            out.push_str(&format!(
                "  - `{}`: contract `{}` ({}); discharge `{}` ({}); reach `{}` ({}); witness `{}` ({})\n",
                evidence.obligation.key,
                evidence.contract.state,
                one_line(&evidence.contract.summary),
                evidence.discharge.state,
                one_line(&evidence.discharge.summary),
                evidence.reach.state,
                one_line(&evidence.reach.summary),
                evidence.witness.state,
                one_line(&evidence.witness.summary)
            ));
        }
    }
}

fn card_hypothesis(card: &ReviewCard) -> String {
    format!(
        "static `{}` ReviewCard for `{}`; confirm with external evidence before treating it as observed runtime behavior",
        card.class.as_str(),
        one_line(&card.operation.expression)
    )
}

fn card_confirmation_step(card: &ReviewCard) -> String {
    if let Some(command) = card.next_action.verify_commands.first() {
        return format!(
            "build/run `{}` first for this card, then attach a matching receipt if it confirms the route",
            command
        );
    }
    if let Some(route) = card.routes.first() {
        return format!(
            "use the `{}` route in this witness plan to derive a focused repro or human review before upgrading confidence",
            route.kind.as_str()
        );
    }
    "derive a focused confirmation from `unsafe-review explain` and human review before upgrading confidence".to_string()
}

fn route_can_show(kind: WitnessKind) -> &'static str {
    match kind {
        WitnessKind::Miri => {
            "a concrete Miri run may expose undefined behavior on the exercised pure-Rust path when that path is supported"
        }
        WitnessKind::CargoCareful => {
            "a cheaper runtime check may catch several unsafe precondition violations on the exercised path"
        }
        WitnessKind::AddressSanitizer
        | WitnessKind::MemorySanitizer
        | WitnessKind::ThreadSanitizer
        | WitnessKind::LeakSanitizer => {
            "a configured sanitizer run may expose runtime memory or thread diagnostics on exercised inputs"
        }
        WitnessKind::Loom | WitnessKind::Shuttle => {
            "a focused model may explore scheduler interleavings for the encoded concurrency invariant"
        }
        WitnessKind::Kani | WitnessKind::Crux => {
            "a focused proof harness may discharge the encoded assertions under its modeled assumptions"
        }
        WitnessKind::HumanDeepReview | WitnessKind::Unsupported => {
            "reviewer-owned contract reasoning, assumptions, and missing context"
        }
    }
}

fn route_cannot_prove(kind: WitnessKind) -> &'static str {
    match kind {
        WitnessKind::Miri | WitnessKind::CargoCareful => {
            "arbitrary callers, full path coverage, repo safety, UB-free status, or site execution without a matching receipt"
        }
        WitnessKind::AddressSanitizer
        | WitnessKind::MemorySanitizer
        | WitnessKind::ThreadSanitizer
        | WitnessKind::LeakSanitizer => {
            "all inputs, platforms, foreign code behavior, repo safety, or UB-free status"
        }
        WitnessKind::Loom | WitnessKind::Shuttle => {
            "interleavings outside the modeled harness, unsupported state, repo safety, or UB-free status"
        }
        WitnessKind::Kani | WitnessKind::Crux => {
            "callers, bounds, contracts, or code paths not encoded in the harness"
        }
        WitnessKind::HumanDeepReview | WitnessKind::Unsupported => {
            "memory safety, UB-free status, site execution, or witness success"
        }
    }
}

fn receipt_hint(card: &ReviewCard, route: &WitnessRoute) -> String {
    let command = route.command.as_deref().unwrap_or("<command>");
    match route.kind {
        WitnessKind::Miri => receipt_import_hint(card, "import-miri", None, command),
        WitnessKind::CargoCareful => receipt_import_hint(card, "import-careful", None, command),
        WitnessKind::AddressSanitizer
        | WitnessKind::MemorySanitizer
        | WitnessKind::ThreadSanitizer
        | WitnessKind::LeakSanitizer => {
            receipt_import_hint(card, "import-sanitizer", Some(route.kind), command)
        }
        WitnessKind::Loom | WitnessKind::Shuttle => {
            receipt_import_hint(card, "import-concurrency", Some(route.kind), command)
        }
        WitnessKind::Kani | WitnessKind::Crux => {
            receipt_import_hint(card, "import-proof", Some(route.kind), command)
        }
        WitnessKind::HumanDeepReview | WitnessKind::Unsupported => {
            "no saved-output receipt applies until an external witness run exists; keep review assumptions and limits explicit".to_string()
        }
    }
}

fn receipt_import_hint(
    card: &ReviewCard,
    import_kind: &str,
    tool: Option<WitnessKind>,
    command: &str,
) -> String {
    let tool_arg = tool
        .map(|kind| format!(" --tool {}", kind.as_str()))
        .unwrap_or_default();
    format!(
        "after running this outside `unsafe-review`, import saved output with `unsafe-review receipt {} {}{} --log <file> --author <owner> --recorded-at <utc> --expires-at <date> --command \"{}\"`",
        import_kind, card.id, tool_arg, command
    )
}

fn missing_summary(card: &ReviewCard) -> String {
    if card.missing.is_empty() {
        return "No missing evidence recorded".to_string();
    }
    card.missing
        .iter()
        .map(|missing| missing.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

fn hazard_summary(card: &ReviewCard) -> String {
    if card.hazards.is_empty() {
        return "none recorded".to_string();
    }
    card.hazards
        .iter()
        .map(|hazard| format!("`{}`", hazard.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use std::path::PathBuf;

    #[test]
    fn witness_plan_routes_cards_without_claiming_execution() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let rendered = render(&output);

        assert!(rendered.contains("# unsafe-review witness plan"));
        assert!(rendered.contains("## Route groups"));
        assert!(rendered.contains("### Miri / cargo-careful"));
        assert!(rendered.contains("Concrete runtime evidence is path-specific"));
        assert!(rendered.contains("Operation: `unsafe { ptr.cast::<Header>().read() }`"));
        assert!(rendered.contains("Route: `miri`"));
        assert!(rendered.contains("What it can show"));
        assert!(rendered.contains("What it cannot prove"));
        assert!(rendered.contains("cargo +nightly miri test read_header"));
        assert!(rendered.contains("unsafe-review receipt import-miri"));
        assert!(rendered.contains("unsafe-review receipt import-careful"));
        assert!(rendered.contains("Next action: Add or expose"));
        assert!(rendered.contains("- Hypothesis to confirm: static `guard_missing` ReviewCard"));
        assert!(rendered.contains(
            "- Confirmation step: build/run `cargo +nightly miri test read_header` first"
        ));
        assert!(rendered.contains("Verify command"));
        assert!(rendered.contains("Missing visible local guard"));
        assert!(rendered.contains("does not run Miri"));
        assert!(rendered.contains("not UB-free status"));
        Ok(())
    }

    #[test]
    fn witness_plan_empty_output_uses_standard_advisory_wording() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let rendered = render(&output);

        assert!(rendered.contains(NO_CHANGED_GAPS_MESSAGE));
        assert!(rendered.contains(NO_CHANGED_GAPS_LIMITATION));
        assert!(rendered.contains("No witness routes are recommended"));
        assert!(!rendered.contains("All clear"));
        Ok(())
    }

    #[test]
    fn witness_plan_shows_imported_receipts_and_remaining_gaps() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment_receipted")?;
        let rendered = render(&output);

        assert!(rendered.contains("Imported miri receipt"));
        assert!(rendered.contains("expires_at: 2026-08-18"));
        assert!(rendered.contains("Missing visible local guard"));
        assert!(rendered.contains("Receipt hint"));
        assert!(rendered.contains("not a Miri result unless a witness receipt is attached"));
        Ok(())
    }

    #[test]
    fn witness_plan_groups_concurrency_routes_with_limits() -> Result<(), String> {
        let output = fixture_output("unsafe_impl_send")?;
        let rendered = render(&output);

        assert!(rendered.contains("### Loom / Shuttle"));
        assert!(rendered.contains("Concurrency witnesses explore modeled scheduler interleavings"));
        assert!(rendered.contains("Route: `loom`"));
        assert!(rendered.contains("Route: `shuttle`"));
        assert!(rendered.contains("unsafe-review receipt import-concurrency"));
        assert!(rendered.contains("What it cannot prove"));
        assert!(rendered.contains("outside the modeled harness"));
        assert!(!rendered.contains("Miri passed"));
        assert!(!rendered.contains("All clear"));
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
}
