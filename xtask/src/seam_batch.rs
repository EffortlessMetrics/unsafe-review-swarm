//! #2224 seam-batch evaluator.
//!
//! Manifest-only, offline validation of the real-PR seam denominator
//! batches under `docs/accuracy/batches/`. The checker recomputes every
//! reported count from the committed rows and fails closed on anything
//! unaccounted: unmapped cards, unmapped seams, tally disagreements, and
//! hash mismatches on the frozen diffs and outputs.
//!
//! Re-running the analyzer is acquisition, not checking. This command never
//! touches the network.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::{parse_toml_file, workspace_path};

const BATCH_DIRS: &[&str] = &[
    "docs/accuracy/batches/2224-pr1",
    "docs/accuracy/batches/2224-pr2",
];

pub(crate) fn check() -> Result<(), String> {
    for dir in BATCH_DIRS {
        check_dir(&workspace_path(dir))?;
    }
    Ok(())
}

pub(crate) fn check_dir(dir: &Path) -> Result<(), String> {
    let batch = parse_toml_file(&dir.join("batch.toml"))?;
    let seams = parse_toml_file(&dir.join("seams.toml"))?;
    let mapping = parse_toml_file(&dir.join("mapping.toml"))?;

    let cases = parse_cases(&batch)?;
    verify_diff_pins(dir, &cases)?;
    let cards_by_case = verify_output_pins(dir, &mapping)?;

    let tally = evaluate(&batch, &seams, &mapping, &cards_by_case)?;
    let stated = parse_stated_tally(&mapping)?;
    tally.require_equal(&stated)?;

    println!(
        "check-seam-batch: ok batch={} (seams={} matched={} missing={} unknown={} cards={} useful={} quiet={} challenges={}/{})",
        dir.display(),
        tally.expected_seams,
        tally.matched,
        tally.missing,
        tally.unknown,
        tally.emitted_cards,
        tally.useful,
        tally.quiet_cards,
        tally.challenge_found,
        tally.challenge_seams,
    );
    Ok(())
}

struct BatchCase {
    expected_seams: usize,
    diff_file: String,
    diff_sha256: String,
}

fn parse_cases(batch: &toml::Value) -> Result<BTreeMap<String, BatchCase>, String> {
    let tables = batch
        .get("cases")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "batch.toml: missing [[cases]] array".to_string())?;
    if tables.is_empty() {
        return Err("batch.toml: [[cases]] is empty".to_string());
    }
    let mut cases = BTreeMap::new();
    for (idx, item) in tables.iter().enumerate() {
        let context = format!("batch.toml cases[{idx}]");
        let table = item
            .as_table()
            .ok_or_else(|| format!("{context} must be a table"))?;
        let id = table
            .get("id")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string id"))?;
        let expected_seams = table
            .get("expected_seams")
            .and_then(toml::Value::as_integer)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or_else(|| format!("{context} missing non-negative expected_seams"))?;
        let diff_file = table
            .get("diff_file")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string diff_file"))?
            .to_string();
        let diff_sha256 = table
            .get("diff_sha256")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string diff_sha256"))?
            .to_string();
        if cases
            .insert(
                id.to_string(),
                BatchCase {
                    expected_seams,
                    diff_file,
                    diff_sha256,
                },
            )
            .is_some()
        {
            return Err(format!("{context} duplicate case id `{id}`"));
        }
    }
    Ok(cases)
}

fn verify_diff_pins(dir: &Path, cases: &BTreeMap<String, BatchCase>) -> Result<(), String> {
    for (id, case) in cases {
        let context = format!("batch.toml case `{id}`");
        let file_name = Path::new(&case.diff_file)
            .file_name()
            .ok_or_else(|| format!("{context} diff_file has no file name"))?;
        let bytes = std::fs::read(dir.join("diffs").join(file_name))
            .map_err(|err| format!("{context} cannot read frozen diff: {err}"))?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = hex_digest(hasher);
        if actual != case.diff_sha256 {
            return Err(format!(
                "{context} frozen diff hash mismatch: manifest={} actual={actual}",
                case.diff_sha256
            ));
        }
    }
    Ok(())
}

fn verify_output_pins(
    dir: &Path,
    mapping: &toml::Value,
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let tables = mapping
        .get("outputs")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "mapping.toml: missing [[outputs]] array".to_string())?;
    if tables.is_empty() {
        return Err("mapping.toml: [[outputs]] is empty".to_string());
    }
    let mut cards_by_case: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (idx, item) in tables.iter().enumerate() {
        let context = format!("mapping.toml outputs[{idx}]");
        let table = item
            .as_table()
            .ok_or_else(|| format!("{context} must be a table"))?;
        let case = table
            .get("case")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string case"))?;
        let file = table
            .get("file")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string file"))?;
        reject_escaping_path(file, &context)?;
        let expected = table
            .get("sha256")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing sha256"))?;
        let bytes = std::fs::read(dir.join(file))
            .map_err(|err| format!("{context} cannot read frozen output `{file}`: {err}"))?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let actual = hex_digest(hasher);
        if actual != expected {
            return Err(format!(
                "{context} frozen output hash mismatch for case `{case}`: manifest={expected} actual={actual}"
            ));
        }
        let parsed: serde_json::Value = serde_json::from_slice(&bytes)
            .map_err(|err| format!("{context} frozen output `{file}` is not JSON: {err}"))?;
        let cards = parsed
            .get("cards")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| format!("{context} frozen output `{file}` has no cards array"))?;
        let mut ids = Vec::new();
        for (card_idx, card) in cards.iter().enumerate() {
            let id = card
                .get("id")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| format!("{context} card[{card_idx}] has no string id"))?;
            ids.push(id.to_string());
        }
        if cards_by_case.insert(case.to_string(), ids).is_some() {
            return Err(format!("{context} duplicate outputs case `{case}`"));
        }
    }
    Ok(cards_by_case)
}

fn reject_escaping_path(file: &str, context: &str) -> Result<(), String> {
    if file.starts_with('/') || file.contains('\\') || file.split('/').any(|part| part == "..") {
        return Err(format!(
            "{context} output file must stay inside the batch dir: {file}"
        ));
    }
    Ok(())
}

fn hex_digest(hasher: Sha256) -> String {
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Tally {
    expected_seams: usize,
    matched: usize,
    missing: usize,
    unknown: usize,
    family_correct: usize,
    family_total: usize,
    over_credited: usize,
    obligations_correct: usize,
    obligations_total: usize,
    emitted_cards: usize,
    duplicates: usize,
    wrongly_surfaced: usize,
    useful: usize,
    quiet_cards: usize,
    challenge_seams: usize,
    challenge_found: usize,
}

impl Tally {
    fn require_equal(&self, stated: &Tally) -> Result<(), String> {
        let pairs = [
            ("expected_seams", self.expected_seams, stated.expected_seams),
            ("matched", self.matched, stated.matched),
            ("missing", self.missing, stated.missing),
            ("unknown", self.unknown, stated.unknown),
            ("family_correct", self.family_correct, stated.family_correct),
            ("family_total", self.family_total, stated.family_total),
            ("over_credited", self.over_credited, stated.over_credited),
            (
                "obligations_correct",
                self.obligations_correct,
                stated.obligations_correct,
            ),
            (
                "obligations_total",
                self.obligations_total,
                stated.obligations_total,
            ),
            ("emitted_cards", self.emitted_cards, stated.emitted_cards),
            ("duplicates", self.duplicates, stated.duplicates),
            (
                "wrongly_surfaced",
                self.wrongly_surfaced,
                stated.wrongly_surfaced,
            ),
            ("useful", self.useful, stated.useful),
            ("quiet_cards", self.quiet_cards, stated.quiet_cards),
            (
                "challenge_seams",
                self.challenge_seams,
                stated.challenge_seams,
            ),
            (
                "challenge_found",
                self.challenge_found,
                stated.challenge_found,
            ),
        ];
        for (field, recomputed, quoted) in pairs {
            if recomputed != quoted {
                return Err(format!(
                    "mapping.toml [tally] {field} disagrees with its rows: stated={quoted} recomputed={recomputed}"
                ));
            }
        }
        Ok(())
    }
}

fn parse_stated_tally(mapping: &toml::Value) -> Result<Tally, String> {
    let table = mapping
        .get("tally")
        .and_then(toml::Value::as_table)
        .ok_or_else(|| "mapping.toml: missing [tally] table".to_string())?;
    let get = |field: &str| {
        table
            .get(field)
            .and_then(toml::Value::as_integer)
            .and_then(|count| usize::try_from(count).ok())
            .ok_or_else(|| format!("mapping.toml [tally] missing non-negative {field}"))
    };
    Ok(Tally {
        expected_seams: get("expected_seams")?,
        matched: get("matched")?,
        missing: get("missing")?,
        unknown: get("unknown")?,
        family_correct: get("family_correct")?,
        family_total: get("family_total")?,
        over_credited: get("over_credited")?,
        obligations_correct: get("obligations_correct")?,
        obligations_total: get("obligations_total")?,
        emitted_cards: get("emitted_cards")?,
        duplicates: get("duplicates")?,
        wrongly_surfaced: get("wrongly_surfaced")?,
        useful: get("useful")?,
        quiet_cards: get("quiet_cards")?,
        challenge_seams: get("challenge_seams")?,
        challenge_found: get("challenge_found")?,
    })
}

fn evaluate(
    batch: &toml::Value,
    seams: &toml::Value,
    mapping: &toml::Value,
    cards_by_case: &BTreeMap<String, Vec<String>>,
) -> Result<Tally, String> {
    let cases = parse_cases(batch)?;

    // Source-first ordering is machine-checked: the inventory must predate
    // any analyzer output consultation.
    match seams.get("analyzer_outputs_consulted") {
        Some(toml::Value::Boolean(false)) => {}
        _ => {
            return Err("seams.toml must record analyzer_outputs_consulted = false".to_string());
        }
    }

    let seam_rows = seams
        .get("seams")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "seams.toml: missing [[seams]] array".to_string())?;
    let mut seam_case: BTreeMap<String, String> = BTreeMap::new();
    let mut per_case_seams: BTreeMap<String, usize> = BTreeMap::new();
    for (idx, item) in seam_rows.iter().enumerate() {
        let context = format!("seams.toml seams[{idx}]");
        let table = item
            .as_table()
            .ok_or_else(|| format!("{context} must be a table"))?;
        let id = table
            .get("id")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string id"))?;
        let case = table
            .get("case")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string case"))?;
        if !cases.contains_key(case) {
            return Err(format!("{context} names unknown batch case `{case}`"));
        }
        if seam_case.insert(id.to_string(), case.to_string()).is_some() {
            return Err(format!("{context} duplicate seam id `{id}`"));
        }
        *per_case_seams.entry(case.to_string()).or_default() += 1;
    }
    for (case, batch_case) in &cases {
        let have = per_case_seams.get(case).copied().unwrap_or(0);
        if have != batch_case.expected_seams {
            return Err(format!(
                "case `{case}` declares expected_seams={} but the inventory holds {have} rows",
                batch_case.expected_seams
            ));
        }
    }

    let result_rows = mapping
        .get("seam_results")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "mapping.toml: missing [[seam_results]] array".to_string())?;
    let mut outcomes: BTreeMap<String, &toml::map::Map<String, toml::Value>> = BTreeMap::new();
    for (idx, item) in result_rows.iter().enumerate() {
        let context = format!("mapping.toml seam_results[{idx}]");
        let table = item
            .as_table()
            .ok_or_else(|| format!("{context} must be a table"))?;
        let seam = table
            .get("seam")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string seam"))?;
        if !seam_case.contains_key(seam) {
            return Err(format!("{context} names unknown seam `{seam}`"));
        }
        if outcomes.insert(seam.to_string(), table).is_some() {
            return Err(format!("{context} duplicate result for seam `{seam}`"));
        }
    }
    let mut tally = Tally {
        expected_seams: seam_case.len(),
        ..Default::default()
    };
    let mut matched_primary: BTreeSet<String> = BTreeSet::new();
    for (seam, case) in &seam_case {
        let table = outcomes
            .get(seam)
            .ok_or_else(|| format!("seam `{seam}` has no row in mapping.toml [[seam_results]]"))?;
        let context = format!("mapping.toml result for seam `{seam}`");
        let outcome = table
            .get("outcome")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string outcome"))?;
        let cards = table
            .get("cards")
            .and_then(toml::Value::as_array)
            .ok_or_else(|| format!("{context} missing cards array"))?;
        let known: BTreeSet<&str> = cards_by_case
            .get(case)
            .map(|ids| ids.iter().map(String::as_str).collect())
            .unwrap_or_default();
        for card in cards {
            let id = card
                .as_str()
                .ok_or_else(|| format!("{context} cards holds a non-string"))?;
            if !known.contains(id) {
                return Err(format!(
                    "{context} maps unknown card `{id}` for case `{case}`"
                ));
            }
        }
        let flag = |field: &str| {
            table
                .get(field)
                .and_then(toml::Value::as_bool)
                .ok_or_else(|| format!("{context} missing boolean {field}"))
        };
        match outcome {
            "match" => {
                if cards.is_empty() {
                    return Err(format!("{context} outcome match maps no cards"));
                }
                tally.matched += 1;
                tally.family_total += 1;
                tally.obligations_total += 1;
                if flag("family_correct")? {
                    tally.family_correct += 1;
                }
                if flag("over_credited")? {
                    tally.over_credited += 1;
                }
                if flag("obligations_correct")? {
                    tally.obligations_correct += 1;
                }
                matched_primary.insert(seam.clone());
            }
            "missing" | "unknown" => {
                if !cards.is_empty() {
                    return Err(format!("{context} outcome {outcome} must map no cards"));
                }
                if outcome == "missing" {
                    tally.missing += 1;
                } else {
                    tally.unknown += 1;
                }
            }
            other => {
                return Err(format!(
                    "{context} outcome must be match, missing, or unknown, not `{other}`"
                ));
            }
        }
    }

    let card_rows = mapping
        .get("card_results")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| "mapping.toml: missing [[card_results]] array".to_string())?;
    let mut seen_cards: BTreeSet<(String, String)> = BTreeSet::new();
    for (idx, item) in card_rows.iter().enumerate() {
        let context = format!("mapping.toml card_results[{idx}]");
        let table = item
            .as_table()
            .ok_or_else(|| format!("{context} must be a table"))?;
        let card = table
            .get("card")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string card"))?;
        let case = table
            .get("case")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string case"))?;
        let known: BTreeSet<&str> = cards_by_case
            .get(case)
            .map(|ids| ids.iter().map(String::as_str).collect())
            .unwrap_or_default();
        if !known.contains(card) {
            return Err(format!(
                "{context} names card `{card}` not emitted for case `{case}`"
            ));
        }
        if !seen_cards.insert((case.to_string(), card.to_string())) {
            return Err(format!("{context} duplicate row for card `{card}`"));
        }
        let disposition = table
            .get("disposition")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{context} missing string disposition"))?;
        let of_seam = table
            .get("of_seam")
            .and_then(toml::Value::as_str)
            .unwrap_or("");
        let useful = table
            .get("useful")
            .and_then(toml::Value::as_bool)
            .ok_or_else(|| format!("{context} missing boolean useful"))?;
        let challenge = table
            .get("inventory_challenge")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false);
        let case_is_quiet = cases
            .get(case)
            .map(|batch_case| batch_case.expected_seams == 0)
            .unwrap_or(false);
        if case_is_quiet {
            tally.quiet_cards += 1;
            if disposition != "wrongly_surfaced" {
                return Err(format!(
                    "{context} quiet-control case `{case}` cards must be wrongly_surfaced"
                ));
            }
        }
        match disposition {
            "matched" | "duplicate" => {
                if of_seam.is_empty() {
                    return Err(format!("{context} disposition {disposition} needs of_seam"));
                }
                if !matched_primary.contains(of_seam) {
                    return Err(format!(
                        "{context} references seam `{of_seam}` with no match outcome"
                    ));
                }
                if seam_case.get(of_seam).map(String::as_str) != Some(case) {
                    return Err(format!(
                        "{context} references seam `{of_seam}` from another case"
                    ));
                }
                if disposition == "duplicate" {
                    tally.duplicates += 1;
                }
            }
            "wrongly_surfaced" => {
                if !of_seam.is_empty() {
                    return Err(format!(
                        "{context} wrongly_surfaced must leave of_seam empty"
                    ));
                }
                tally.wrongly_surfaced += 1;
            }
            other => {
                return Err(format!(
                    "{context} disposition must be matched, duplicate, or wrongly_surfaced, not `{other}`"
                ));
            }
        }
        if useful {
            tally.useful += 1;
        }
        if challenge {
            tally.challenge_seams += 1;
            if useful {
                tally.challenge_found += 1;
            }
        }
        tally.emitted_cards += 1;
    }
    let total_emitted: usize = cards_by_case.values().map(Vec::len).sum();
    if seen_cards.len() != total_emitted {
        return Err(format!(
            "mapping.toml accounts {} of {total_emitted} emitted cards; every card needs a row",
            seen_cards.len()
        ));
    }
    // Every match-outcome seam needs at least one primary (non-duplicate)
    // card; duplicates alone do not establish the find.
    for seam in &matched_primary {
        let primaries = card_rows
            .iter()
            .filter_map(toml::Value::as_table)
            .filter(|table| {
                table.get("of_seam").and_then(toml::Value::as_str) == Some(seam)
                    && table.get("disposition").and_then(toml::Value::as_str) == Some("matched")
            })
            .count();
        if primaries == 0 {
            return Err(format!(
                "seam `{seam}` has match outcome but no primary matched card"
            ));
        }
    }
    Ok(tally)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI_BATCH: &str = r#"
schema_version = "seam-batch/v1"
[[cases]]
id = "loud"
expected_seams = 2
diff_file = "diffs/loud.diff"
diff_sha256 = "00"
[[cases]]
id = "quiet"
expected_seams = 0
diff_file = "diffs/quiet.diff"
diff_sha256 = "00"
"#;

    const MINI_SEAMS: &str = r#"
schema_version = "seam-batch-seams/v1"
analyzer_outputs_consulted = false
[[seams]]
id = "loud-S1"
case = "loud"
operation_family = "raw_pointer_write"
[[seams]]
id = "loud-S2"
case = "loud"
operation_family = "ffi"
"#;

    const MINI_MAPPING: &str = r#"
schema_version = "seam-batch-mapping/v1"
[[seam_results]]
seam = "loud-S1"
outcome = "match"
family_correct = true
over_credited = false
obligations_correct = true
cards = ["card-a"]
[[seam_results]]
seam = "loud-S2"
outcome = "missing"
family_correct = false
over_credited = false
obligations_correct = false
cards = []
[[card_results]]
card = "card-a"
case = "loud"
disposition = "matched"
of_seam = "loud-S1"
useful = true
inventory_challenge = false
[[card_results]]
card = "card-b"
case = "loud"
disposition = "duplicate"
of_seam = "loud-S1"
useful = false
inventory_challenge = false
[[card_results]]
card = "card-c"
case = "loud"
disposition = "wrongly_surfaced"
of_seam = ""
useful = true
inventory_challenge = true
[tally]
expected_seams = 2
matched = 1
missing = 1
unknown = 0
family_correct = 1
family_total = 1
over_credited = 0
obligations_correct = 1
obligations_total = 1
emitted_cards = 3
duplicates = 1
wrongly_surfaced = 1
useful = 2
quiet_cards = 0
challenge_seams = 1
challenge_found = 1
"#;

    fn mini_cards() -> BTreeMap<String, Vec<String>> {
        BTreeMap::from([
            (
                "loud".to_string(),
                vec![
                    "card-a".to_string(),
                    "card-b".to_string(),
                    "card-c".to_string(),
                ],
            ),
            ("quiet".to_string(), Vec::new()),
        ])
    }

    fn parse(text: &str) -> Result<toml::Value, String> {
        text.parse::<toml::Table>()
            .map(toml::Value::Table)
            .map_err(|err| format!("test TOML parses: {err}"))
    }

    fn must_fail<T>(result: Result<T, String>, what: &str) -> Result<String, String> {
        match result {
            Err(err) => Ok(err),
            Ok(_) => Err(format!("{what} should have failed")),
        }
    }

    #[test]
    fn known_counts_evaluate_clean() -> Result<(), String> {
        let tally = evaluate(
            &parse(MINI_BATCH)?,
            &parse(MINI_SEAMS)?,
            &parse(MINI_MAPPING)?,
            &mini_cards(),
        )?;
        tally.require_equal(&parse_stated_tally(&parse(MINI_MAPPING)?)?)?;
        assert_eq!(tally.matched, 1);
        assert_eq!(tally.missing, 1);
        assert_eq!(tally.emitted_cards, 3);
        Ok(())
    }

    #[test]
    fn dropped_seam_moves_recall_tally() -> Result<(), String> {
        // Remove loud-S2 and its result while keeping the stated tally: the
        // gate must fail naming the moved count.
        let seams = parse(&MINI_SEAMS.replace(
            "[[seams]]\nid = \"loud-S2\"\ncase = \"loud\"\noperation_family = \"ffi\"\n",
            "",
        ))?;
        let mapping = parse(&MINI_MAPPING.replace(
            "[[seam_results]]\nseam = \"loud-S2\"\noutcome = \"missing\"\nfamily_correct = false\nover_credited = false\nobligations_correct = false\ncards = []\n",
            "",
        ))?;
        let batch = parse(&MINI_BATCH.replace("expected_seams = 2", "expected_seams = 1"))?;
        let tally = evaluate(&batch, &seams, &mapping, &mini_cards())?;
        assert_eq!(tally.expected_seams, 1);
        assert_eq!(tally.missing, 0);
        let err = must_fail(
            tally.require_equal(&parse_stated_tally(&parse(MINI_MAPPING)?)?),
            "stale tally",
        )?;
        assert!(err.contains("expected_seams"), "{err}");
        Ok(())
    }

    #[test]
    fn changed_label_breaks_tally_agreement() -> Result<(), String> {
        // Flip loud-S1 to missing (re-dispositioning its cards so the mapping
        // stays structurally valid) while the stated tally still claims it:
        // agreement must fail on the moved field.
        let mapping = parse(
            &MINI_MAPPING
                .replace(
                    "seam = \"loud-S1\"\noutcome = \"match\"",
                    "seam = \"loud-S1\"\noutcome = \"missing\"",
                )
                .replace("cards = [\"card-a\"]", "cards = []")
                .replace(
                    "card = \"card-a\"\ncase = \"loud\"\ndisposition = \"matched\"\nof_seam = \"loud-S1\"",
                    "card = \"card-a\"\ncase = \"loud\"\ndisposition = \"wrongly_surfaced\"\nof_seam = \"\"",
                )
                .replace(
                    "card = \"card-b\"\ncase = \"loud\"\ndisposition = \"duplicate\"\nof_seam = \"loud-S1\"",
                    "card = \"card-b\"\ncase = \"loud\"\ndisposition = \"wrongly_surfaced\"\nof_seam = \"\"",
                ),
        )?;
        let tally = evaluate(
            &parse(MINI_BATCH)?,
            &parse(MINI_SEAMS)?,
            &mapping,
            &mini_cards(),
        )?;
        assert_eq!(tally.matched, 0);
        let err = must_fail(
            tally.require_equal(&parse_stated_tally(&parse(MINI_MAPPING)?)?),
            "moved tally",
        )?;
        assert!(err.contains("matched"), "{err}");
        Ok(())
    }

    #[test]
    fn partial_run_surfaces_as_unknown() -> Result<(), String> {
        // An unknown outcome with matching tally passes and stays visible.
        let mapping = parse(
            &MINI_MAPPING
                .replace(
                    "seam = \"loud-S2\"\noutcome = \"missing\"",
                    "seam = \"loud-S2\"\noutcome = \"unknown\"",
                )
                .replace("missing = 1\nunknown = 0", "missing = 0\nunknown = 1"),
        )?;
        let tally = evaluate(
            &parse(MINI_BATCH)?,
            &parse(MINI_SEAMS)?,
            &mapping,
            &mini_cards(),
        )?;
        assert_eq!(tally.unknown, 1);
        tally.require_equal(&parse_stated_tally(&mapping)?)?;
        Ok(())
    }

    #[test]
    fn unaccounted_card_fails_closed() -> Result<(), String> {
        let mut cards = mini_cards();
        cards
            .get_mut("loud")
            .ok_or("missing loud case")?
            .push("card-ghost".to_string());
        let err = must_fail(
            evaluate(
                &parse(MINI_BATCH)?,
                &parse(MINI_SEAMS)?,
                &parse(MINI_MAPPING)?,
                &cards,
            ),
            "ghost card",
        )?;
        assert!(
            err.contains("card-ghost") || err.contains("accounts"),
            "{err}"
        );
        Ok(())
    }

    #[test]
    fn quiet_case_cards_must_be_wrongly_surfaced() -> Result<(), String> {
        let mut cards = mini_cards();
        cards
            .get_mut("quiet")
            .ok_or("missing quiet case")?
            .push("card-q".to_string());
        let mapping = parse(&format!(
            "{MINI_MAPPING}[[card_results]]\ncard = \"card-q\"\ncase = \"quiet\"\ndisposition = \"matched\"\nof_seam = \"loud-S1\"\nuseful = true\ninventory_challenge = false\n"
        ))?;
        let err = must_fail(
            evaluate(&parse(MINI_BATCH)?, &parse(MINI_SEAMS)?, &mapping, &cards),
            "quiet violation",
        )?;
        assert!(err.contains("quiet"), "{err}");
        Ok(())
    }
}
