mod card;
mod header;

use crate::api::AnalyzeOutput;
use crate::output::{
    NO_CHANGED_GAPS_LIMITATION, NO_CHANGED_GAPS_MESSAGE, REVIEWCARD_TRUST_BOUNDARY,
};

pub(crate) fn render(output: &AnalyzeOutput) -> String {
    let mut out = String::new();
    header::render_header(&mut out, output);

    if output.cards.is_empty() {
        push_line(&mut out, NO_CHANGED_GAPS_MESSAGE);
        push_line(&mut out, NO_CHANGED_GAPS_LIMITATION);
        return out;
    }

    for card in &output.cards {
        card::render_card(&mut out, card);
    }

    out.push_str("Trust boundary: ");
    out.push_str(REVIEWCARD_TRUST_BOUNDARY);
    out.push('\n');
    out
}

fn push_line(out: &mut String, line: &str) {
    out.push_str(line);
    out.push('\n');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{AnalysisMode, AnalyzeInput, DiffSource, PolicyMode, Scope, analyze};
    use crate::domain::ReviewCard;
    use crate::util::path_display;
    use std::collections::{BTreeMap, BTreeSet};
    use std::fmt::Debug;
    use std::path::PathBuf;

    const RAW_POINTER_ALIGNMENT_CARD_ID: &str = "UR-raw-pointer-alignment-fixture-src-lib-rs-read-header-operation-raw_pointer_read-cast-header-8a1362456e39-pointer_validity-c1";
    const STABLE_BYTE_SAB_CARD_ID: &str = "UR-stable-byte-sab-borrowed-slice-fixture-src-lib-rs-textdecoder-sab-decode-operation-stable_byte_source_sab_race-is-shared-array-buffer-4ee9ff2124f6-stable_byte_source-c1";

    #[derive(Debug, PartialEq, Eq)]
    struct ParsedHeader {
        fields: BTreeMap<String, String>,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct ParsedCard {
        heading: String,
        labels: BTreeMap<String, String>,
        sections: BTreeMap<String, Vec<String>>,
        top_level_rows: Vec<String>,
    }

    #[derive(Debug, PartialEq, Eq)]
    struct ParsedHuman {
        header: ParsedHeader,
        cards: Vec<ParsedCard>,
        trust_boundaries: Vec<String>,
    }

    #[test]
    fn human_output_names_conditions_evidence_and_routes() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let source_card = select_card(&output.cards, RAW_POINTER_ALIGNMENT_CARD_ID)?;
        let parsed = parse_human(&render(&output), &[RAW_POINTER_ALIGNMENT_CARD_ID])?;
        let rendered_card = select_parsed_card(&parsed.cards, RAW_POINTER_ALIGNMENT_CARD_ID)?;

        expect_label(
            rendered_card,
            "operation",
            &source_card.operation.expression,
        )?;
        expect_section(
            rendered_card,
            "hazards",
            source_card
                .hazards
                .iter()
                .map(|hazard| hazard.as_str().to_string())
                .collect(),
        )?;
        expect_section(
            rendered_card,
            "required safety conditions",
            source_card
                .obligations
                .iter()
                .map(|obligation| obligation.description.clone())
                .collect(),
        )?;
        expect_section(
            rendered_card,
            "obligation evidence",
            source_card
                .obligation_evidence
                .iter()
                .map(|evidence| {
                    format!(
                        "{}: contract {}, guard {}, reach {}, witness {}",
                        evidence.obligation.key,
                        evidence.contract.state,
                        evidence.discharge.state,
                        evidence.reach.state,
                        evidence.witness.state
                    )
                })
                .collect(),
        )?;
        expect_section(
            rendered_card,
            "witness routes",
            expected_witness_routes(source_card),
        )?;
        expect_label(
            rendered_card,
            "reach note",
            "static reach evidence only; it does not prove site execution.",
        )?;
        Ok(())
    }

    #[test]
    fn human_empty_output_uses_standard_advisory_wording() -> Result<(), String> {
        let output = fixture_output("safe_code_no_cards")?;
        let rendered = render(&output);
        let parsed = parse_human(&rendered, &[])?;

        expect_eq("empty parsed card count", parsed.cards.len(), 0)?;
        expect_eq(
            "empty message count",
            exact_line_count(&rendered, NO_CHANGED_GAPS_MESSAGE),
            1,
        )?;
        expect_eq(
            "empty limitation count",
            exact_line_count(&rendered, NO_CHANGED_GAPS_LIMITATION),
            1,
        )?;
        expect_eq(
            "empty card trust boundary count",
            parsed.trust_boundaries.len(),
            0,
        )?;
        expect_eq(
            "empty all-clear count",
            exact_line_count(&rendered, "All clear"),
            0,
        )?;
        Ok(())
    }

    #[test]
    fn human_header_projects_the_five_named_summary_counts() -> Result<(), String> {
        let output = fixture_output("stable_byte_sab_borrowed_slice")?;
        let parsed = parse_human(&render(&output), &[])?;

        expect_header_count(&parsed.header, "cards", output.summary.cards)?;
        expect_header_count(
            &parsed.header,
            "open gaps",
            output.summary.open_actionable_gaps,
        )?;
        expect_header_count(
            &parsed.header,
            "contract_missing",
            output.summary.contract_missing,
        )?;
        expect_header_count(
            &parsed.header,
            "guard_missing",
            output.summary.guard_missing,
        )?;
        expect_header_count(
            &parsed.header,
            "witness gaps",
            output.summary.guarded_unwitnessed,
        )?;

        for &omitted in HEADER_MOVEMENT_AND_READINESS_ROWS {
            require(
                !parsed.header.fields.contains_key(omitted),
                format!("human header unexpectedly emitted `{omitted}`"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn human_cards_project_selected_review_card_fields_structurally() -> Result<(), String> {
        for (fixture, card_id, stable_byte_sub_class) in [
            ("raw_pointer_alignment", RAW_POINTER_ALIGNMENT_CARD_ID, None),
            (
                "stable_byte_sab_borrowed_slice",
                STABLE_BYTE_SAB_CARD_ID,
                Some("sab-race"),
            ),
        ] {
            let output = fixture_output(fixture)?;
            let source_card = select_card(&output.cards, card_id)?;
            let parsed = parse_human(&render(&output), &[card_id])?;
            let rendered_card = select_parsed_card(&parsed.cards, card_id)?;

            expect_eq(
                "card trust boundary count",
                parsed.trust_boundaries.len(),
                1,
            )?;
            expect_eq(
                "card trust boundary",
                parsed.trust_boundaries.first().map(String::as_str),
                Some(REVIEWCARD_TRUST_BOUNDARY),
            )?;

            let expected_heading = format!(
                "{} {}:{}",
                source_card.class.as_str().to_uppercase(),
                path_display(&source_card.site.location.file),
                source_card.site.location.line
            );
            expect_eq(
                "card heading",
                rendered_card.heading.as_str(),
                expected_heading.as_str(),
            )?;
            expect_eq(
                "selected card id label count",
                row_count(rendered_card, "id"),
                1,
            )?;
            expect_label(rendered_card, "id", card_id)?;
            expect_label(
                rendered_card,
                "operation_family",
                source_card.operation.family.as_str(),
            )?;
            expect_section(
                rendered_card,
                "missing",
                source_card
                    .missing
                    .iter()
                    .map(|missing| missing.message.clone())
                    .collect(),
            )?;
            expect_label(rendered_card, "next", &source_card.next_action.summary)?;
            expect_optional_section(
                rendered_card,
                "verify",
                &source_card.next_action.verify_commands,
            )?;
            expect_optional_label(
                rendered_card,
                "stable_byte_sub_class",
                stable_byte_sub_class,
            )?;
            expect_top_level_shape(rendered_card, source_card, stable_byte_sub_class.is_some())?;
        }
        Ok(())
    }

    #[test]
    fn human_card_omits_named_canonical_rows_and_bounded_aliases() -> Result<(), String> {
        let output = fixture_output("raw_pointer_alignment")?;
        let parsed = parse_human(&render(&output), &[RAW_POINTER_ALIGNMENT_CARD_ID])?;
        let card = select_parsed_card(&parsed.cards, RAW_POINTER_ALIGNMENT_CARD_ID)?;

        for &omitted in CANONICAL_OMITTED_ROWS.iter().chain(BOUNDED_OMITTED_ALIASES) {
            require(
                !card.top_level_rows.iter().any(|row| row == omitted),
                format!("human card unexpectedly emitted top-level row `{omitted}`"),
            )?;
        }
        Ok(())
    }

    #[test]
    fn human_optional_rows_follow_their_independently_derived_inputs() -> Result<(), String> {
        let raw_output = fixture_output("raw_pointer_alignment")?;
        let raw_parsed = parse_human(&render(&raw_output), &[RAW_POINTER_ALIGNMENT_CARD_ID])?;
        let raw_card = select_parsed_card(&raw_parsed.cards, RAW_POINTER_ALIGNMENT_CARD_ID)?;
        expect_optional_label(raw_card, "stable_byte_sub_class", None)?;

        let stable_output = fixture_output("stable_byte_sab_borrowed_slice")?;
        let stable_parsed = parse_human(&render(&stable_output), &[STABLE_BYTE_SAB_CARD_ID])?;
        let stable_card = select_parsed_card(&stable_parsed.cards, STABLE_BYTE_SAB_CARD_ID)?;
        expect_optional_label(stable_card, "stable_byte_sub_class", Some("sab-race"))?;

        let mut without_verify = raw_output;
        let selected = select_card_mut(&mut without_verify.cards, RAW_POINTER_ALIGNMENT_CARD_ID)?;
        selected.next_action.verify_commands.clear();
        let without_verify_parsed =
            parse_human(&render(&without_verify), &[RAW_POINTER_ALIGNMENT_CARD_ID])?;
        let without_verify_card =
            select_parsed_card(&without_verify_parsed.cards, RAW_POINTER_ALIGNMENT_CARD_ID)?;
        expect_optional_section(without_verify_card, "verify", &[])?;
        Ok(())
    }

    const HEADER_MOVEMENT_AND_READINESS_ROWS: &[&str] = &[
        "new_gaps",
        "worsened_gaps",
        "improved_gaps",
        "resolved_gaps",
        "inherited_gaps",
        "new",
        "worsened",
        "improved",
        "resolved",
        "inherited",
        "coverage_movement_summary",
        "coverage_movement",
        "movement",
        "agent_readiness",
        "agent_lsp_readiness",
        "readiness",
    ];

    const CANONICAL_OMITTED_ROWS: &[&str] = &[
        "column",
        "end_line",
        "end_column",
        "owner",
        "kind",
        "visibility",
        "public_api_surface",
        "changed",
        "snippet",
        "priority",
        "confidence",
        "severity",
        "actionability",
        "baseline_state",
        "outcome_movement",
        "contract_coverage",
        "guard_coverage",
        "test_reach_coverage",
        "witness_receipt_coverage",
        "manual_context",
        "agent_readiness",
        "comment_plan_status",
        "unsafe_sites",
        "new_gaps",
        "worsened_gaps",
        "improved_gaps",
        "resolved_gaps",
        "inherited_gaps",
        "receipt_status",
        "declaration_summary_group",
        "target_feature_summary_group",
    ];

    const BOUNDED_OMITTED_ALIASES: &[&str] = &[
        "end",
        "end_range",
        "range",
        "public_api",
        "baseline",
        "outcome",
        "coverage",
        "readiness",
        "agent_lsp_readiness",
        "coverage_movement_summary",
        "surfacing_disposition",
        "repair_applicability",
        "freshness_identity",
        "editor_action_contract",
        "receipt",
        "receipts",
        "group",
        "groups",
    ];

    fn parse_human(rendered: &str, selected_ids: &[&str]) -> Result<ParsedHuman, String> {
        let header_line = rendered
            .lines()
            .find(|line| line.starts_with("cards: "))
            .ok_or_else(|| "human output is missing its cards summary".to_string())?;
        let header = parse_header(header_line);
        let cards = selected_ids
            .iter()
            .map(|id| parse_selected_card(rendered, id))
            .collect::<Result<Vec<_>, _>>()?;
        let trust_boundaries = rendered
            .lines()
            .filter_map(|line| line.strip_prefix("Trust boundary: "))
            .map(str::to_string)
            .collect();
        Ok(ParsedHuman {
            header,
            cards,
            trust_boundaries,
        })
    }

    fn parse_header(line: &str) -> ParsedHeader {
        let fields = line
            .split(", ")
            .filter_map(|field| field.split_once(": "))
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        ParsedHeader { fields }
    }

    fn parse_selected_card(rendered: &str, id: &str) -> Result<ParsedCard, String> {
        let lines: Vec<_> = rendered.lines().collect();
        let id_line = format!("  id: {id}");
        let id_indices: Vec<_> = lines
            .iter()
            .enumerate()
            .filter_map(|(index, line)| (*line == id_line).then_some(index))
            .collect();
        expect_eq("selected card id row count", id_indices.len(), 1)?;
        let id_index = id_indices
            .first()
            .copied()
            .ok_or_else(|| format!("human output is missing selected card `{id}`"))?;
        let heading = id_index
            .checked_sub(1)
            .and_then(|index| lines.get(index))
            .ok_or_else(|| format!("selected card `{id}` is missing its heading"))?
            .to_string();
        let block_end = lines[id_index..]
            .iter()
            .position(|line| line.is_empty())
            .map(|offset| id_index + offset)
            .unwrap_or(lines.len());

        let mut labels = BTreeMap::new();
        let mut sections: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut top_level_rows = Vec::new();
        let mut active_section: Option<String> = None;
        for line in &lines[id_index..block_end] {
            if let Some(field) = exact_two_space_field(line) {
                let Some((name, value)) = field.split_once(':') else {
                    active_section = None;
                    continue;
                };
                top_level_rows.push(name.to_string());
                if value.is_empty() {
                    if labels.contains_key(name) || sections.contains_key(name) {
                        return Err(format!(
                            "selected card `{id}` repeats top-level row `{name}`"
                        ));
                    }
                    sections.insert(name.to_string(), Vec::new());
                    active_section = Some(name.to_string());
                } else if let Some(value) = value.strip_prefix(' ') {
                    if sections.contains_key(name)
                        || labels.insert(name.to_string(), value.to_string()).is_some()
                    {
                        return Err(format!(
                            "selected card `{id}` repeats top-level row `{name}`"
                        ));
                    }
                    active_section = None;
                } else {
                    active_section = None;
                }
                continue;
            }
            if let (Some(section), Some(item)) =
                (active_section.as_ref(), line.strip_prefix("    "))
            {
                if let Some(items) = sections.get_mut(section) {
                    items.push(item.strip_prefix("- ").unwrap_or(item).to_string());
                }
            }
        }
        Ok(ParsedCard {
            heading,
            labels,
            sections,
            top_level_rows,
        })
    }

    fn exact_two_space_field(line: &str) -> Option<&str> {
        line.strip_prefix("  ")
            .filter(|field| !field.starts_with(' '))
    }

    fn expect_header_count(
        header: &ParsedHeader,
        name: &str,
        expected: usize,
    ) -> Result<(), String> {
        let actual = header
            .fields
            .get(name)
            .ok_or_else(|| format!("human header is missing `{name}`"))?
            .parse::<usize>()
            .map_err(|error| format!("human header `{name}` is not a count: {error}"))?;
        expect_eq(name, actual, expected)
    }

    fn expect_top_level_shape(
        rendered: &ParsedCard,
        source: &ReviewCard,
        has_stable_byte_sub_class: bool,
    ) -> Result<(), String> {
        expect_eq(
            "human card unique top-level row count",
            rendered.top_level_rows.len(),
            rendered.labels.len() + rendered.sections.len(),
        )?;
        let mut expected_labels = BTreeSet::from([
            "id",
            "operation",
            "operation_family",
            "proof_path",
            "contract",
            "discharge",
            "reach",
            "reach note",
            "next",
        ]);
        if has_stable_byte_sub_class {
            expected_labels.insert("stable_byte_sub_class");
        }
        let actual_labels: BTreeSet<_> = rendered.labels.keys().map(String::as_str).collect();
        expect_eq("human card label keys", actual_labels, expected_labels)?;

        let mut expected_sections =
            BTreeSet::from(["hazards", "required safety conditions", "missing"]);
        if !source.obligation_evidence.is_empty() {
            expected_sections.insert("obligation evidence");
        }
        if !source.routes.is_empty() {
            expected_sections.insert("witness routes");
        }
        if !source.next_action.verify_commands.is_empty() {
            expected_sections.insert("verify");
        }
        let actual_sections: BTreeSet<_> = rendered.sections.keys().map(String::as_str).collect();
        expect_eq(
            "human card section keys",
            actual_sections,
            expected_sections,
        )
    }

    fn expected_witness_routes(card: &ReviewCard) -> Vec<String> {
        let mut expected = Vec::new();
        for route in &card.routes {
            expected.push(format!("{}: {}", route.kind.as_str(), route.reason));
            if let Some(command) = &route.command {
                expected.push(format!("  command: {command}"));
            }
        }
        expected
    }

    fn select_card<'a>(cards: &'a [ReviewCard], id: &str) -> Result<&'a ReviewCard, String> {
        let matches: Vec<_> = cards.iter().filter(|card| card.id.0 == id).collect();
        expect_eq("analyzed exact-id card count", matches.len(), 1)?;
        matches
            .first()
            .copied()
            .ok_or_else(|| format!("analyzed card `{id}` disappeared after exact-id selection"))
    }

    fn select_card_mut<'a>(
        cards: &'a mut [ReviewCard],
        id: &str,
    ) -> Result<&'a mut ReviewCard, String> {
        let count = cards.iter().filter(|card| card.id.0 == id).count();
        expect_eq("mutable exact-id card count", count, 1)?;
        cards
            .iter_mut()
            .find(|card| card.id.0 == id)
            .ok_or_else(|| format!("analyzed card `{id}` disappeared after exact-id selection"))
    }

    fn select_parsed_card<'a>(cards: &'a [ParsedCard], id: &str) -> Result<&'a ParsedCard, String> {
        let matches: Vec<_> = cards
            .iter()
            .filter(|card| card.labels.get("id").map(String::as_str) == Some(id))
            .collect();
        expect_eq("rendered exact-id card count", matches.len(), 1)?;
        matches
            .first()
            .copied()
            .ok_or_else(|| format!("rendered card `{id}` disappeared after exact-id selection"))
    }

    fn row_count(card: &ParsedCard, name: &str) -> usize {
        card.top_level_rows
            .iter()
            .filter(|row| *row == name)
            .count()
    }

    fn expect_label(card: &ParsedCard, name: &str, expected: &str) -> Result<(), String> {
        let actual = card
            .labels
            .get(name)
            .ok_or_else(|| format!("human card is missing label `{name}`"))?;
        expect_eq(name, actual.as_str(), expected)
    }

    fn expect_optional_label(
        card: &ParsedCard,
        name: &str,
        expected: Option<&str>,
    ) -> Result<(), String> {
        expect_eq(name, card.labels.get(name).map(String::as_str), expected)
    }

    fn expect_section(card: &ParsedCard, name: &str, expected: Vec<String>) -> Result<(), String> {
        let actual = card
            .sections
            .get(name)
            .ok_or_else(|| format!("human card is missing section `{name}`"))?;
        expect_eq(name, actual, &expected)
    }

    fn expect_optional_section(
        card: &ParsedCard,
        name: &str,
        expected: &[String],
    ) -> Result<(), String> {
        match (expected.is_empty(), card.sections.get(name)) {
            (true, None) => Ok(()),
            (false, Some(actual)) => expect_eq(name, actual.as_slice(), expected),
            (true, Some(_)) => Err(format!(
                "human card emitted empty optional section `{name}`"
            )),
            (false, None) => Err(format!(
                "human card omitted non-empty optional section `{name}`"
            )),
        }
    }

    fn exact_line_count(rendered: &str, expected: &str) -> usize {
        rendered.lines().filter(|line| *line == expected).count()
    }

    fn require(condition: bool, message: impl Into<String>) -> Result<(), String> {
        if condition {
            Ok(())
        } else {
            Err(message.into())
        }
    }

    fn expect_eq<T>(context: &str, actual: T, expected: T) -> Result<(), String>
    where
        T: Debug + PartialEq,
    {
        if actual == expected {
            Ok(())
        } else {
            Err(format!(
                "{context} mismatch: actual={actual:?}, expected={expected:?}"
            ))
        }
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
