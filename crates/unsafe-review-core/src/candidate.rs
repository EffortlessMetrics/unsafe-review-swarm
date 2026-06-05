use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const MANUAL_CANDIDATE_SCHEMA_VERSION: &str = "manual-candidate/v1";
pub const MANUAL_CANDIDATE_INDEX_SCHEMA_VERSION: &str = "manual-candidates/v1";
const MANUAL_CANDIDATE_TRUST_BOUNDARY: &str = "manual candidate; not analyzer-discovered; not witness execution; not proof of memory safety; not UB-free status; not Miri-clean status; not site-execution proof; not policy readiness";
const MANUAL_CANDIDATE_TRUST_BOUNDARY_PHRASES: [&str; 8] = [
    "manual candidate",
    "not analyzer-discovered",
    "not witness execution",
    "not proof",
    "not UB-free",
    "not Miri-clean",
    "not site-execution",
    "not policy",
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCandidate {
    pub schema_version: String,
    pub id: String,
    pub title: String,
    pub location: ManualCandidateLocation,
    pub operation_family: String,
    pub unsafe_operation: String,
    pub invariant: String,
    pub safe_caller: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oracle_map: Option<ManualCandidateOracleMap>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stable_byte: Option<ManualCandidateStableByte>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proof_mode: Option<ManualCandidateProofMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix_boundary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pr_aperture: Option<String>,
    #[serde(default)]
    pub evidence: Vec<ManualCandidateEvidence>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fix_options: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_targets: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub do_not_touch: Vec<String>,
    pub trust_boundary: String,
    #[serde(default = "manual_source")]
    pub source: String,
    #[serde(default)]
    pub manual_candidate: bool,
    #[serde(default)]
    pub analyzer_discovered: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCandidateLocation {
    pub file: PathBuf,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCandidateProofMode {
    pub kind: String,
    pub system_bun_expected: String,
    pub mutation_required: bool,
    pub miri_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCandidateOracleMap {
    pub rust_seam: String,
    pub oracle_language: String,
    pub oracle_path: PathBuf,
    pub oracle_kind: String,
    pub coverage_confidence: String,
    pub limitation: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCandidateStableByte {
    pub class: String,
    pub source: String,
    pub sink: String,
    pub hazard: String,
    pub observable: String,
    pub proof_required: String,
    pub suggested_fix_boundary: String,
    pub pr_aperture: String,
    pub ledger_state: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCandidateEvidence {
    pub kind: String,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub limitation: Option<String>,
}

impl ManualCandidate {
    pub fn from_json_str(text: &str) -> Result<Self, String> {
        let mut candidate: Self = serde_json::from_str(text)
            .map_err(|err| format!("parse manual candidate JSON failed: {err}"))?;
        candidate.normalize();
        candidate.validate()?;
        Ok(candidate)
    }

    pub fn to_pretty_json(&self) -> Result<String, String> {
        let mut rendered = serde_json::to_string_pretty(self)
            .map_err(|err| format!("render manual candidate JSON failed: {err}"))?;
        rendered.push('\n');
        Ok(rendered)
    }

    fn normalize(&mut self) {
        self.source = "manual".to_string();
        self.manual_candidate = true;
        self.analyzer_discovered = false;
        if self.trust_boundary.trim().is_empty() {
            self.trust_boundary = MANUAL_CANDIDATE_TRUST_BOUNDARY.to_string();
        }
    }

    fn validate(&self) -> Result<(), String> {
        require_eq(
            "schema_version",
            &self.schema_version,
            MANUAL_CANDIDATE_SCHEMA_VERSION,
        )?;
        validate_candidate_id(&self.id)?;
        require_nonempty("title", &self.title)?;
        if self.location.file.as_os_str().is_empty() {
            return Err("manual candidate location.file must not be empty".to_string());
        }
        if self.location.line == 0 {
            return Err("manual candidate location.line must be 1-based".to_string());
        }
        require_nonempty("operation_family", &self.operation_family)?;
        require_nonempty("unsafe_operation", &self.unsafe_operation)?;
        require_nonempty("invariant", &self.invariant)?;
        require_nonempty("safe_caller", &self.safe_caller)?;
        if let Some(oracle_map) = &self.oracle_map {
            oracle_map.validate()?;
        }
        if let Some(stable_byte) = &self.stable_byte {
            stable_byte.validate(
                self.proof_mode.as_ref(),
                self.fix_boundary.as_deref(),
                self.pr_aperture.as_deref(),
            )?;
        }
        if let Some(proof_mode) = &self.proof_mode {
            proof_mode.validate()?;
        }
        require_optional_nonempty("fix_boundary", self.fix_boundary.as_deref())?;
        require_optional_nonempty("pr_aperture", self.pr_aperture.as_deref())?;
        validate_manual_candidate_trust_boundary(&self.trust_boundary)?;
        require_nonempty_items("fix_options", &self.fix_options)?;
        require_nonempty_items("test_targets", &self.test_targets)?;
        require_nonempty_items("do_not_touch", &self.do_not_touch)?;
        for evidence in &self.evidence {
            if !is_known_evidence_kind(&evidence.kind) {
                return Err(format!(
                    "manual candidate evidence kind `{}` is not supported",
                    evidence.kind
                ));
            }
            require_optional_nonempty("evidence.command", evidence.command.as_deref())?;
            require_optional_nonempty("evidence.limitation", evidence.limitation.as_deref())?;
        }
        Ok(())
    }
}

impl ManualCandidateOracleMap {
    fn validate(&self) -> Result<(), String> {
        require_nonempty("oracle_map.rust_seam", &self.rust_seam)?;
        require_nonempty("oracle_map.oracle_language", &self.oracle_language)?;
        if self.oracle_path.as_os_str().is_empty() {
            return Err("manual candidate oracle_map.oracle_path must not be empty".to_string());
        }
        require_nonempty("oracle_map.oracle_kind", &self.oracle_kind)?;
        require_nonempty("oracle_map.coverage_confidence", &self.coverage_confidence)?;
        require_nonempty("oracle_map.limitation", &self.limitation)?;
        for required in [
            "not witness execution",
            "site-execution proof",
            "memory-safety proof",
        ] {
            if !self.limitation.to_ascii_lowercase().contains(required) {
                return Err(format!(
                    "manual candidate oracle_map.limitation must include `{required}`"
                ));
            }
        }
        Ok(())
    }
}

impl ManualCandidateStableByte {
    fn validate(
        &self,
        proof_mode: Option<&ManualCandidateProofMode>,
        fix_boundary: Option<&str>,
        pr_aperture: Option<&str>,
    ) -> Result<(), String> {
        if !is_known_stable_byte_class(&self.class) {
            return Err(format!(
                "manual candidate stable_byte.class `{}` is not supported",
                self.class
            ));
        }
        require_nonempty("stable_byte.source", &self.source)?;
        require_nonempty("stable_byte.sink", &self.sink)?;
        require_nonempty("stable_byte.hazard", &self.hazard)?;
        if !is_known_stable_byte_observable(&self.observable) {
            return Err(format!(
                "manual candidate stable_byte.observable `{}` is not supported",
                self.observable
            ));
        }
        if !is_known_proof_mode_kind(&self.proof_required) {
            return Err(format!(
                "manual candidate stable_byte.proof_required `{}` is not supported",
                self.proof_required
            ));
        }
        if let Some(proof_mode) = proof_mode {
            if self.proof_required != proof_mode.kind {
                return Err(format!(
                    "manual candidate stable_byte.proof_required `{}` must match proof_mode.kind `{}`",
                    self.proof_required, proof_mode.kind
                ));
            }
        }
        require_nonempty(
            "stable_byte.suggested_fix_boundary",
            &self.suggested_fix_boundary,
        )?;
        if let Some(fix_boundary) = fix_boundary {
            if self.suggested_fix_boundary != fix_boundary {
                return Err(format!(
                    "manual candidate stable_byte.suggested_fix_boundary must match fix_boundary `{fix_boundary}`"
                ));
            }
        }
        require_nonempty("stable_byte.pr_aperture", &self.pr_aperture)?;
        if let Some(pr_aperture) = pr_aperture {
            if self.pr_aperture != pr_aperture {
                return Err(format!(
                    "manual candidate stable_byte.pr_aperture must match pr_aperture `{pr_aperture}`"
                ));
            }
        }
        if !is_known_stable_byte_ledger_state(&self.ledger_state) {
            return Err(format!(
                "manual candidate stable_byte.ledger_state `{}` is not supported",
                self.ledger_state
            ));
        }
        Ok(())
    }
}

impl ManualCandidateProofMode {
    fn validate(&self) -> Result<(), String> {
        if !is_known_proof_mode_kind(&self.kind) {
            return Err(format!(
                "manual candidate proof_mode.kind `{}` is not supported",
                self.kind
            ));
        }
        if !is_known_system_bun_expected(&self.system_bun_expected) {
            return Err(format!(
                "manual candidate proof_mode.system_bun_expected `{}` is not supported",
                self.system_bun_expected
            ));
        }
        Ok(())
    }
}

pub fn read_manual_candidate(path: &Path) -> Result<ManualCandidate, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("read manual candidate {} failed: {err}", path.display()))?;
    ManualCandidate::from_json_str(&text)
}

pub fn load_manual_candidate(root: &Path, id: &str) -> Result<Option<ManualCandidate>, String> {
    validate_candidate_id(id)?;
    let path = manual_candidate_path(root, id);
    if !path.exists() {
        return Ok(None);
    }
    read_manual_candidate(&path).map(Some)
}

pub fn load_manual_candidates(root: &Path) -> Result<Vec<ManualCandidate>, String> {
    let dir = root.join(".unsafe-review").join("candidates");
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let entries =
        fs::read_dir(&dir).map_err(|err| format!("read {} failed: {err}", dir.display()))?;
    let mut candidates = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|err| format!("read_dir entry failed: {err}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        candidates.push(read_manual_candidate(&path)?);
    }
    candidates.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(candidates)
}

pub fn manual_candidate_path(root: &Path, id: &str) -> PathBuf {
    root.join(".unsafe-review")
        .join("candidates")
        .join(format!("{id}.json"))
}

pub fn render_manual_candidate_explain(candidate: &ManualCandidate) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "# unsafe-review manual candidate `{}`\n\n",
        candidate.id
    ));
    out.push_str(&format!("{}\n\n", candidate.title));
    out.push_str("## Source\n\n");
    out.push_str("- Source: `manual`\n");
    out.push_str("- Manual candidate: `true`\n");
    out.push_str("- Analyzer-discovered: `false`\n");
    out.push_str(&format!(
        "- Location: `{}`:{}\n",
        candidate.location.file.display(),
        candidate.location.line
    ));
    out.push_str(&format!(
        "- Operation family: `{}`\n",
        candidate.operation_family
    ));
    out.push_str(&format!(
        "- Unsafe operation: `{}`\n\n",
        candidate.unsafe_operation
    ));
    out.push_str("## Manual invariant\n\n");
    out.push_str(&candidate.invariant);
    out.push_str("\n\n");
    out.push_str("## Safe caller\n\n");
    out.push_str(&candidate.safe_caller);
    out.push_str("\n\n");
    render_candidate_handoff_markdown(&mut out, candidate);
    render_candidate_evidence_markdown(&mut out, candidate);
    out.push_str("## Next action\n\n");
    out.push_str("Review the manual candidate, preserve the external evidence packet, and import receipts only when they match this manual candidate ID.\n\n");
    out.push_str("## Trust boundary\n\n");
    out.push_str(&candidate.trust_boundary);
    out.push('\n');
    out
}

pub fn render_manual_candidate_context(candidate: &ManualCandidate) -> Result<String, String> {
    let mut value = serde_json::json!({
        "schema_version": "manual-candidate-context/v1",
        "id": candidate.id,
        "source": "manual",
        "manual_candidate": true,
        "analyzer_discovered": false,
        "title": candidate.title,
        "location": {
            "file": candidate.location.file.display().to_string(),
            "line": candidate.location.line,
        },
        "operation_family": candidate.operation_family,
        "unsafe_operation": candidate.unsafe_operation,
        "invariant": candidate.invariant,
        "safe_caller": candidate.safe_caller,
        "oracle_map": candidate.oracle_map.as_ref(),
        "stable_byte": candidate.stable_byte.as_ref(),
        "implementer_handoff": manual_candidate_implementer_handoff(candidate),
        "evidence": candidate.evidence.iter().map(|evidence| serde_json::json!({
            "kind": evidence.kind,
            "path": evidence.path.as_ref().map(|path| path.display().to_string()),
            "summary": evidence.summary,
            "command": evidence.command,
            "limitation": evidence.limitation,
        })).collect::<Vec<_>>(),
        "allowed_actions": [
            "review the manual route and invariant",
            "attach receipts that reference this manual candidate ID",
            "project explain/context/witness-plan with the manual marker preserved"
        ],
        "do_not_do": [
            "do not label this analyzer-discovered",
            "do not claim proof, UB-free status, Miri-clean status, or site execution",
            "do not execute witnesses automatically"
        ],
        "trust_boundary": candidate.trust_boundary,
    });
    if let Some(object) = value.as_object_mut() {
        if candidate.stable_byte.is_none() {
            object.remove("stable_byte");
        }
        if candidate.oracle_map.is_none() {
            object.remove("oracle_map");
        }
        if let Some(proof_mode) = &candidate.proof_mode {
            object.insert("proof_mode".to_string(), serde_json::json!(proof_mode));
        }
        if let Some(fix_boundary) = &candidate.fix_boundary {
            object.insert("fix_boundary".to_string(), serde_json::json!(fix_boundary));
        }
        if let Some(pr_aperture) = &candidate.pr_aperture {
            object.insert("pr_aperture".to_string(), serde_json::json!(pr_aperture));
        }
        insert_non_empty_json_array(object, "fix_options", &candidate.fix_options);
        insert_non_empty_json_array(object, "test_targets", &candidate.test_targets);
        insert_non_empty_json_array(object, "do_not_touch", &candidate.do_not_touch);
    }
    let mut rendered = serde_json::to_string_pretty(&value)
        .map_err(|err| format!("render manual candidate context failed: {err}"))?;
    rendered.push('\n');
    Ok(rendered)
}

pub fn render_manual_candidate_witness_plan(candidate: &ManualCandidate) -> String {
    let mut out = String::new();
    out.push_str("# unsafe-review manual candidate witness plan\n\n");
    out.push_str("This plan is derived from a manually imported candidate. It does not run witnesses and does not make the candidate analyzer-discovered.\n\n");
    out.push_str(&format!("## `{}`\n\n", candidate.id));
    out.push_str(&format!("Title: {}\n\n", candidate.title));
    out.push_str(&format!(
        "Location: `{}`:{}\n\n",
        candidate.location.file.display(),
        candidate.location.line
    ));
    out.push_str(&format!(
        "Operation family: `{}`\n\n",
        candidate.operation_family
    ));
    out.push_str(&format!(
        "Unsafe operation: `{}`\n\n",
        candidate.unsafe_operation
    ));
    out.push_str(&format!("Invariant: {}\n\n", candidate.invariant));
    out.push_str(&format!("Safe caller: {}\n\n", candidate.safe_caller));
    render_candidate_handoff_markdown(&mut out, candidate);
    render_candidate_evidence_markdown(&mut out, candidate);
    out.push_str("## Receipt hints\n\n");
    out.push_str(&format!(
        "- `unsafe-review receipt template {}` can create metadata for a focused manual review receipt.\n",
        candidate.id
    ));
    out.push_str("- Tool-specific receipt import is allowed only when the saved log actually belongs to this manual candidate ID.\n\n");
    out.push_str("## Trust boundary\n\n");
    out.push_str(&candidate.trust_boundary);
    out.push('\n');
    out
}

fn render_candidate_evidence_markdown(out: &mut String, candidate: &ManualCandidate) {
    out.push_str("## External evidence\n\n");
    if candidate.evidence.is_empty() {
        out.push_str("- No external evidence paths recorded.\n\n");
        return;
    }
    for evidence in &candidate.evidence {
        out.push_str(&format!("- `{}`", evidence.kind));
        if let Some(path) = &evidence.path {
            out.push_str(&format!(": `{}`", path.display()));
        }
        if let Some(summary) = &evidence.summary {
            out.push_str(&format!(" - {summary}"));
        }
        if let Some(command) = &evidence.command {
            out.push_str(&format!("; command `{command}`"));
        }
        if let Some(limitation) = &evidence.limitation {
            out.push_str(&format!("; limitation: {limitation}"));
        }
        out.push('\n');
    }
    out.push('\n');
}

fn render_candidate_handoff_markdown(out: &mut String, candidate: &ManualCandidate) {
    out.push_str("## Implementer handoff\n\n");
    out.push_str(&format!(
        "- Inspect: `{}`\n",
        manual_candidate_location_text(candidate)
    ));
    out.push_str(&format!(
        "- Route: `{}` -> `{}`\n",
        candidate.safe_caller, candidate.unsafe_operation
    ));
    out.push_str(&format!("- Invariant at risk: {}\n", candidate.invariant));
    if let Some(oracle_map) = &candidate.oracle_map {
        out.push_str(&format!(
            "- Oracle map: Rust seam `{}` -> `{}` oracle `{}` (`{}`; confidence: `{}`; limitation: {})\n",
            oracle_map.rust_seam,
            oracle_map.oracle_language,
            oracle_map.oracle_path.display(),
            oracle_map.oracle_kind,
            oracle_map.coverage_confidence,
            oracle_map.limitation
        ));
    }
    if let Some(proof_mode) = &candidate.proof_mode {
        out.push_str(&format!(
            "- Proof mode: `{}` (system Bun expected: `{}`; mutation required: `{}`; Miri/model required: `{}`)\n",
            proof_mode.kind,
            proof_mode.system_bun_expected,
            proof_mode.mutation_required,
            proof_mode.miri_required
        ));
    }
    if let Some(stable_byte) = &candidate.stable_byte {
        out.push_str(&format!(
            "- Stable-byte class: `{}` (observable: `{}`; proof required: `{}`; ledger state: `{}`)\n",
            stable_byte.class,
            stable_byte.observable,
            stable_byte.proof_required,
            stable_byte.ledger_state
        ));
        out.push_str(&format!(
            "- Stable-byte source: `{}` -> sink: `{}`\n",
            stable_byte.source, stable_byte.sink
        ));
        out.push_str(&format!("- Stable-byte hazard: {}\n", stable_byte.hazard));
    }
    if let Some(fix_boundary) = &candidate.fix_boundary {
        out.push_str(&format!("- Fix boundary: {fix_boundary}\n"));
    }
    if let Some(pr_aperture) = &candidate.pr_aperture {
        out.push_str(&format!("- PR aperture: {pr_aperture}\n"));
    }
    render_string_list(out, "Fix options", &candidate.fix_options);
    render_string_list(out, "Test targets", &candidate.test_targets);
    render_string_list(out, "Do not touch", &candidate.do_not_touch);
    out.push_str("- Next: confirm the route, preserve or add concrete evidence for the invariant, and attach receipts only when they target this manual candidate ID.\n");
    out.push_str("- Stop line: stop before source edits if the route no longer matches this manual candidate, or if the repair would broaden into unrelated unsafe sites.\n\n");
}

pub fn manual_candidate_implementer_handoff(candidate: &ManualCandidate) -> serde_json::Value {
    let mut value = serde_json::json!({
        "target": {
            "file": candidate.location.file.display().to_string(),
            "line": candidate.location.line,
            "location_text": manual_candidate_location_text(candidate),
        },
        "route": {
            "safe_caller": candidate.safe_caller,
            "unsafe_operation": candidate.unsafe_operation,
            "operation_family": candidate.operation_family,
        },
        "invariant_at_risk": candidate.invariant,
        "oracle_map": candidate.oracle_map.as_ref(),
        "stable_byte": candidate.stable_byte.as_ref(),
        "external_evidence": candidate.evidence.iter().map(|evidence| serde_json::json!({
            "kind": evidence.kind,
            "path": evidence.path.as_ref().map(|path| path.display().to_string()),
            "summary": evidence.summary,
            "command": evidence.command,
            "limitation": evidence.limitation,
        })).collect::<Vec<_>>(),
        "suggested_next_steps": manual_candidate_suggested_next_steps(candidate),
        "non_goals": manual_candidate_non_goals(candidate),
        "stop_condition": "stop before source edits if the route no longer matches this manual candidate, or if the repair would broaden into unrelated unsafe sites",
    });
    if let Some(object) = value.as_object_mut() {
        if candidate.stable_byte.is_none() {
            object.remove("stable_byte");
        }
        if candidate.oracle_map.is_none() {
            object.remove("oracle_map");
        }
        if let Some(proof_mode) = &candidate.proof_mode {
            object.insert("proof_mode".to_string(), serde_json::json!(proof_mode));
        }
        if let Some(fix_boundary) = &candidate.fix_boundary {
            object.insert("fix_boundary".to_string(), serde_json::json!(fix_boundary));
        }
        if let Some(pr_aperture) = &candidate.pr_aperture {
            object.insert("pr_aperture".to_string(), serde_json::json!(pr_aperture));
        }
        insert_non_empty_json_array(object, "fix_options", &candidate.fix_options);
        insert_non_empty_json_array(object, "test_targets", &candidate.test_targets);
        insert_non_empty_json_array(object, "do_not_touch", &candidate.do_not_touch);
    }
    value
}

fn manual_candidate_suggested_next_steps(candidate: &ManualCandidate) -> Vec<String> {
    let mut steps = vec![
        "confirm the file:line and safe caller route before editing".to_string(),
        "preserve or add concrete contract, guard, test, or witness evidence for the invariant"
            .to_string(),
        "attach receipts only when the external run targets this manual candidate ID".to_string(),
    ];
    if !candidate.fix_options.is_empty() {
        steps.push("evaluate the candidate-specific fix options before editing".to_string());
    }
    if candidate.proof_mode.is_some() {
        steps.push(
            "preserve the candidate proof mode and evidence bar before claiming the lane outcome"
                .to_string(),
        );
    }
    if candidate.oracle_map.is_some() {
        steps.push(
            "preserve the cross-language oracle map and limitation when preparing handoffs"
                .to_string(),
        );
    }
    if candidate.stable_byte.is_some() {
        steps.push(
            "preserve the stable-byte class, proof requirement, and ledger metadata in handoffs"
                .to_string(),
        );
    }
    if candidate.fix_boundary.is_some() {
        steps.push("keep the first patch at the candidate-specific fix boundary".to_string());
    }
    if candidate.pr_aperture.is_some() {
        steps.push("keep the PR inside the candidate-specific aperture and stop line".to_string());
    }
    if !candidate.test_targets.is_empty() {
        steps.push(
            "run or preserve the candidate-specific test targets listed in this handoff"
                .to_string(),
        );
    }
    if !candidate.do_not_touch.is_empty() {
        steps.push("respect the candidate-specific do-not-touch notes before editing".to_string());
    }
    steps
}

fn manual_candidate_non_goals(candidate: &ManualCandidate) -> Vec<String> {
    let mut non_goals = vec![
        "do not treat this as analyzer-discovered".to_string(),
        "do not claim proof, UB-free status, Miri-clean status, or site execution".to_string(),
        "do not broaden the task to unrelated unsafe sites".to_string(),
    ];
    non_goals.extend(candidate.do_not_touch.iter().cloned());
    non_goals
}

fn render_string_list(out: &mut String, label: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("- {label}:\n"));
    for item in items {
        out.push_str(&format!("  - {item}\n"));
    }
}

fn insert_non_empty_json_array(
    object: &mut serde_json::Map<String, serde_json::Value>,
    field: &str,
    values: &[String],
) {
    if values.is_empty() {
        return;
    }
    object.insert(field.to_string(), serde_json::json!(values));
}

fn manual_candidate_location_text(candidate: &ManualCandidate) -> String {
    format!(
        "{}:{}",
        candidate.location.file.display(),
        candidate.location.line
    )
}

fn require_eq(field: &str, actual: &str, expected: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "manual candidate {field} must be `{expected}`, got `{actual}`"
        ))
    }
}

fn require_nonempty(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        Err(format!("manual candidate {field} must not be empty"))
    } else {
        Ok(())
    }
}

fn require_nonempty_items(field: &str, values: &[String]) -> Result<(), String> {
    for value in values {
        if value.trim().is_empty() {
            return Err(format!(
                "manual candidate {field} entries must not be empty"
            ));
        }
    }
    Ok(())
}

fn require_optional_nonempty(field: &str, value: Option<&str>) -> Result<(), String> {
    if value.is_some_and(|value| value.trim().is_empty()) {
        Err(format!("manual candidate {field} must not be empty"))
    } else {
        Ok(())
    }
}

fn validate_manual_candidate_trust_boundary(value: &str) -> Result<(), String> {
    require_nonempty("trust_boundary", value)?;
    let value = value.to_ascii_lowercase();
    for required in MANUAL_CANDIDATE_TRUST_BOUNDARY_PHRASES {
        if !value.contains(&required.to_ascii_lowercase()) {
            return Err(format!(
                "manual candidate trust_boundary must include `{required}`"
            ));
        }
    }
    Ok(())
}

fn validate_candidate_id(id: &str) -> Result<(), String> {
    require_nonempty("id", id)?;
    if id != id.trim()
        || id.starts_with("UR-")
        || id.contains('/')
        || id.contains('\\')
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
    {
        return Err("manual candidate id must be path-safe, non-UR, and use only ASCII letters, digits, '.', '_', '-', or ':'".to_string());
    }
    Ok(())
}

fn is_known_evidence_kind(kind: &str) -> bool {
    matches!(
        kind,
        "runtime_witness" | "model" | "source_trace" | "node_parity" | "human_review" | "other"
    )
}

fn is_known_proof_mode_kind(kind: &str) -> bool {
    matches!(
        kind,
        "observable-red-green" | "mutation-plus-miri" | "source-route-only" | "helper-gated"
    )
}

fn is_known_system_bun_expected(value: &str) -> bool {
    matches!(value, "fail" | "nondiscriminating" | "unavailable")
}

fn is_known_stable_byte_class(value: &str) -> bool {
    matches!(
        value,
        "stable-byte-source-rab-async"
            | "stable-byte-source-sab-race"
            | "stable-byte-source-getter-reentry"
            | "stable-byte-source-helper-dependent"
            | "stable-byte-source-pathlike-live-view"
            | "stable-byte-source-native-ffi-read"
    )
}

fn is_known_stable_byte_observable(value: &str) -> bool {
    matches!(value, "yes" | "no" | "source-route-only" | "helper-gated")
}

fn is_known_stable_byte_ledger_state(value: &str) -> bool {
    matches!(
        value,
        "handoff-ready"
            | "fork-draft"
            | "upstream-open"
            | "parked-followup"
            | "merged-upstream"
            | "needs-refresh"
    )
}

fn manual_source() -> String {
    "manual".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn manual_candidate_json_normalizes_manual_source_marker() -> Result<(), String> {
        let candidate = ManualCandidate::from_json_str(example_json())?;

        assert_eq!(candidate.schema_version, MANUAL_CANDIDATE_SCHEMA_VERSION);
        assert_eq!(candidate.id, "R4R2-S001");
        assert_eq!(candidate.source, "manual");
        assert!(candidate.manual_candidate);
        assert!(!candidate.analyzer_discovered);
        assert_eq!(candidate.evidence.len(), 2);
        assert_eq!(
            candidate
                .stable_byte
                .as_ref()
                .map(|stable_byte| stable_byte.class.as_str()),
            Some("stable-byte-source-sab-race")
        );
        assert_eq!(
            candidate.proof_mode.as_ref().map(|mode| mode.kind.as_str()),
            Some("mutation-plus-miri")
        );
        assert_eq!(
            candidate.fix_boundary.as_deref(),
            Some("copy shared bytes before constructing the Rust slice")
        );
        assert_eq!(
            candidate.pr_aperture.as_deref(),
            Some("TextDecoder shared-byte snapshot only; do not rewrite unrelated encodings")
        );
        assert_eq!(
            candidate
                .oracle_map
                .as_ref()
                .map(|oracle_map| oracle_map.oracle_language.as_str()),
            Some("typescript")
        );
        assert_eq!(candidate.fix_options.len(), 1);
        assert_eq!(candidate.test_targets.len(), 1);
        assert_eq!(candidate.do_not_touch.len(), 1);
        let canonical = candidate.to_pretty_json()?;
        assert!(canonical.contains("\"manual_candidate\": true"));
        assert!(canonical.contains("\"analyzer_discovered\": false"));
        assert!(canonical.contains("\"stable_byte\""));
        assert!(canonical.contains("\"proof_mode\""));
        assert!(canonical.contains("\"oracle_map\""));
        assert!(canonical.contains("\"fix_boundary\""));
        assert!(canonical.contains("\"pr_aperture\""));
        assert!(canonical.contains("\"fix_options\""));
        assert!(canonical.contains("\"test_targets\""));
        assert!(canonical.contains("\"do_not_touch\""));
        Ok(())
    }

    #[test]
    fn manual_candidate_rejects_wrong_schema() -> Result<(), String> {
        let err = match ManualCandidate::from_json_str(
            r#"{
              "schema_version": "manual-candidate/v0",
              "id": "R4R2-S001",
              "title": "bad",
              "location": {"file": "src/lib.rs", "line": 1},
              "operation_family": "raw_pointer_read",
              "unsafe_operation": "ptr.read()",
              "invariant": "invariant",
              "safe_caller": "caller",
              "evidence": [],
              "trust_boundary": "manual candidate"
            }"#,
        ) {
            Ok(_) => return Err("manual candidate unexpectedly accepted wrong schema".to_string()),
            Err(err) => err,
        };

        assert!(err.contains("schema_version"));
        Ok(())
    }

    #[test]
    fn manual_candidate_loads_from_default_candidate_dir() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-manual-candidate")?;
        let path = manual_candidate_path(&root, "R4R2-S001");
        fs::create_dir_all(path.parent().ok_or("candidate path missing parent")?)
            .map_err(|err| format!("create candidate dir failed: {err}"))?;
        fs::write(&path, example_json()).map_err(|err| format!("write candidate failed: {err}"))?;

        let candidate = load_manual_candidate(&root, "R4R2-S001")?.ok_or("candidate missing")?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        assert_eq!(candidate.id, "R4R2-S001");
        Ok(())
    }

    #[test]
    fn manual_candidates_load_sorted_from_candidate_dir() -> Result<(), String> {
        let root = unique_temp_dir("unsafe-review-manual-candidates")?;
        let dir = root.join(".unsafe-review").join("candidates");
        fs::create_dir_all(&dir).map_err(|err| format!("create candidate dir failed: {err}"))?;
        fs::write(dir.join("B-002.json"), example_json_with_id("B-002"))
            .map_err(|err| format!("write B candidate failed: {err}"))?;
        fs::write(dir.join("A-001.json"), example_json_with_id("A-001"))
            .map_err(|err| format!("write A candidate failed: {err}"))?;
        fs::write(dir.join("README.md"), "ignored\n")
            .map_err(|err| format!("write ignored file failed: {err}"))?;

        let candidates = load_manual_candidates(&root)?;

        fs::remove_dir_all(&root).map_err(|err| format!("remove temp dir failed: {err}"))?;
        let ids = candidates
            .iter()
            .map(|candidate| candidate.id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["A-001", "B-002"]);
        Ok(())
    }

    #[test]
    fn manual_candidate_rejects_non_path_safe_ids() {
        for id in ["UR-not-counted", "../R4R2-S001", "R4R2 S001", " R4R2-S001"] {
            let err = ManualCandidate::from_json_str(&example_json_with_id(id))
                .err()
                .unwrap_or_default();
            assert!(err.contains("path-safe"), "{id} produced {err}");
        }
    }

    #[test]
    fn manual_candidate_rejects_empty_optional_evidence_metadata() {
        let err = ManualCandidate::from_json_str(&example_json().replace(
            "\"command\": \"bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts\"",
            "\"command\": \"   \"",
        ))
        .err()
        .unwrap_or_default();

        assert!(err.contains("evidence.command"));
    }

    #[test]
    fn manual_candidate_rejects_empty_guidance_entries() {
        let err = ManualCandidate::from_json_str(&example_json().replace(
            "\"copy SharedArrayBuffer-backed bytes before constructing the slice\"",
            "\"   \"",
        ))
        .err()
        .unwrap_or_default();

        assert!(err.contains("fix_options"));
    }

    #[test]
    fn manual_candidate_rejects_unknown_proof_mode() {
        let err = ManualCandidate::from_json_str(&example_json().replace(
            "\"kind\": \"mutation-plus-miri\"",
            "\"kind\": \"sure-from-source\"",
        ))
        .err()
        .unwrap_or_default();

        assert!(err.contains("proof_mode.kind"), "{err}");
    }

    #[test]
    fn manual_candidate_rejects_stable_byte_proof_mode_drift() {
        let err = ManualCandidate::from_json_str(&example_json().replace(
            "\"proof_required\": \"mutation-plus-miri\"",
            "\"proof_required\": \"observable-red-green\"",
        ))
        .err()
        .unwrap_or_default();

        assert!(err.contains("stable_byte.proof_required"), "{err}");
    }

    #[test]
    fn manual_candidate_rejects_stable_byte_fix_boundary_drift() {
        let err = ManualCandidate::from_json_str(&example_json().replacen(
            "\"suggested_fix_boundary\": \"copy shared bytes before constructing the Rust slice\"",
            "\"suggested_fix_boundary\": \"copy bytes somewhere else\"",
            1,
        ))
        .err()
        .unwrap_or_default();

        assert!(err.contains("stable_byte.suggested_fix_boundary"), "{err}");
    }

    #[test]
    fn manual_candidate_rejects_stable_byte_pr_aperture_drift() {
        let err = ManualCandidate::from_json_str(&example_json().replacen(
            "\"pr_aperture\": \"TextDecoder shared-byte snapshot only; do not rewrite unrelated encodings\"",
            "\"pr_aperture\": \"unrelated broad rewrite\"",
            1,
        ))
        .err()
        .unwrap_or_default();

        assert!(err.contains("stable_byte.pr_aperture"), "{err}");
    }

    #[test]
    fn manual_candidate_rejects_empty_fix_boundary() {
        let err = ManualCandidate::from_json_str(&example_json().replace(
            "\"fix_boundary\": \"copy shared bytes before constructing the Rust slice\"",
            "\"fix_boundary\": \"   \"",
        ))
        .err()
        .unwrap_or_default();

        assert!(err.contains("fix_boundary"), "{err}");
    }

    #[test]
    fn manual_candidate_rejects_weak_trust_boundary() {
        let err = ManualCandidate::from_json_str(&example_json().replace(
            "\"trust_boundary\": \"manual candidate; not analyzer-discovered; not witness execution; not proof of memory safety; not UB-free status; not Miri-clean status; not site-execution proof; not policy readiness\"",
            "\"trust_boundary\": \"manual candidate; not analyzer-discovered; not proof of repository safety\"",
        ))
        .err()
        .unwrap_or_default();

        assert!(err.contains("trust_boundary"), "{err}");
        assert!(err.contains("not witness execution"), "{err}");
    }

    #[test]
    fn manual_candidate_rejects_empty_oracle_map_limitation() {
        let err = ManualCandidate::from_json_str(&example_json().replace(
            "\"limitation\": \"oracle map only; not witness execution, site-execution proof, or memory-safety proof\"",
            "\"limitation\": \"   \"",
        ))
        .err()
        .unwrap_or_default();

        assert!(err.contains("oracle_map.limitation"), "{err}");
    }

    #[test]
    fn manual_candidate_explain_context_and_witness_plan_preserve_manual_marker()
    -> Result<(), String> {
        let candidate = ManualCandidate::from_json_str(example_json())?;

        let explain = render_manual_candidate_explain(&candidate);
        let context = render_manual_candidate_context(&candidate)?;
        let witness_plan = render_manual_candidate_witness_plan(&candidate);

        assert!(explain.contains("Source: `manual`"));
        assert!(explain.contains("Analyzer-discovered: `false`"));
        assert!(explain.contains("## Implementer handoff"));
        assert!(explain.contains("Stop line: stop before source edits"));
        assert!(explain.contains("Proof mode: `mutation-plus-miri`"));
        assert!(explain.contains("Stable-byte class: `stable-byte-source-sab-race`"));
        assert!(
            explain.contains("Stable-byte source: `SharedArrayBuffer-backed typed array decode`")
        );
        assert!(explain.contains("Oracle map: Rust seam"));
        assert!(explain.contains("textdecoder-sharedarraybuffer.test.ts"));
        assert!(explain.contains("Fix boundary: copy shared bytes"));
        assert!(explain.contains("PR aperture: TextDecoder shared-byte snapshot"));
        assert!(explain.contains("Fix options"));
        assert!(explain.contains("copy SharedArrayBuffer-backed bytes"));
        assert!(explain.contains("Test targets"));
        assert!(explain.contains("textdecoder-sharedarraybuffer.test.ts"));
        assert!(explain.contains("Do not touch"));
        assert!(explain.contains("TextDecoder unrelated encodings"));
        assert!(explain.contains("command `bun test"));
        assert!(explain.contains("limitation: runtime route evidence only"));
        assert!(context.contains("\"source\": \"manual\""));
        assert!(context.contains("\"manual_candidate\": true"));
        assert!(context.contains("\"implementer_handoff\""));
        assert!(context.contains("\"invariant_at_risk\""));
        assert!(context.contains("\"stable_byte\""));
        assert!(context.contains("\"oracle_map\""));
        assert!(context.contains("\"oracle_language\": \"typescript\""));
        assert!(context.contains("\"ledger_state\": \"handoff-ready\""));
        assert!(context.contains("\"proof_mode\""));
        assert!(context.contains("\"fix_boundary\""));
        assert!(context.contains("\"pr_aperture\""));
        assert!(context.contains("\"command\": \"bun test"));
        assert!(context.contains("\"limitation\": \"runtime route evidence only"));
        assert!(context.contains("\"fix_options\""));
        assert!(context.contains("\"test_targets\""));
        assert!(context.contains("\"do_not_touch\""));
        assert!(context.contains("\"stop_condition\""));
        assert!(witness_plan.contains("manual candidate witness plan"));
        assert!(witness_plan.contains("does not run witnesses"));
        assert!(witness_plan.contains("## Implementer handoff"));
        assert!(witness_plan.contains("Proof mode: `mutation-plus-miri`"));
        assert!(witness_plan.contains("Stable-byte class: `stable-byte-source-sab-race`"));
        assert!(witness_plan.contains("Oracle map: Rust seam"));
        assert!(witness_plan.contains("Fix options"));
        assert!(witness_plan.contains("Test targets"));
        assert!(witness_plan.contains("Do not touch"));
        assert!(witness_plan.contains("command `bun test"));
        Ok(())
    }

    fn example_json() -> &'static str {
        r#"{
          "schema_version": "manual-candidate/v1",
          "id": "R4R2-S001",
          "source": "manual",
          "manual_candidate": true,
          "analyzer_discovered": false,
          "title": "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes",
          "location": {
            "file": "src/runtime/webcore/TextDecoder.rs",
            "line": 237
          },
          "operation_family": "raw_pointer_read",
          "unsafe_operation": "core::slice::from_raw_parts",
          "invariant": "&[u8] memory must not be concurrently mutated",
          "safe_caller": "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))",
          "oracle_map": {
            "rust_seam": "src/runtime/webcore/TextDecoder.rs::decode",
            "oracle_language": "typescript",
            "oracle_path": "test/js/webcore/textdecoder-sharedarraybuffer.test.ts",
            "oracle_kind": "shared-byte-mutation-model",
            "coverage_confidence": "candidate-local",
            "limitation": "oracle map only; not witness execution, site-execution proof, or memory-safety proof"
          },
          "stable_byte": {
            "class": "stable-byte-source-sab-race",
            "source": "SharedArrayBuffer-backed typed array decode",
            "sink": "src/runtime/webcore/TextDecoder.rs slice materialization",
            "hazard": "Rust slice materialization can treat shared JS bytes as stable while JS can mutate the backing storage concurrently",
            "observable": "no",
            "proof_required": "mutation-plus-miri",
            "suggested_fix_boundary": "copy shared bytes before constructing the Rust slice",
            "pr_aperture": "TextDecoder shared-byte snapshot only; do not rewrite unrelated encodings",
            "ledger_state": "handoff-ready"
          },
          "proof_mode": {
            "kind": "mutation-plus-miri",
            "system_bun_expected": "nondiscriminating",
            "mutation_required": true,
            "miri_required": true
          },
          "fix_boundary": "copy shared bytes before constructing the Rust slice",
          "pr_aperture": "TextDecoder shared-byte snapshot only; do not rewrite unrelated encodings",
          "fix_options": [
            "copy SharedArrayBuffer-backed bytes before constructing the slice"
          ],
          "test_targets": [
            "test/js/webcore/textdecoder-sharedarraybuffer.test.ts"
          ],
          "do_not_touch": [
            "Do not rewrite TextDecoder unrelated encodings"
          ],
          "evidence": [
            {
              "kind": "runtime_witness",
              "path": "target/unsafe-scout/textdecoder-shared-race-route.out",
              "summary": "Bun TextDecoder route reaches shared backing bytes through safe JS",
              "command": "bun test test/js/webcore/textdecoder-sharedarraybuffer.test.ts",
              "limitation": "runtime route evidence only; not memory-safety proof and not analyzer-discovered"
            },
            {
              "kind": "model",
              "path": "target/unsafe-scout/miri-textdecoder-shared-slice.out",
              "summary": "Miri model covers shared-slice aliasing shape outside Bun runtime",
              "command": "cargo +nightly miri test textdecoder_shared_slice_model",
              "limitation": "model evidence only; does not prove the Bun site executed under Miri"
            }
          ],
          "trust_boundary": "manual candidate; not analyzer-discovered; not witness execution; not proof of memory safety; not UB-free status; not Miri-clean status; not site-execution proof; not policy readiness"
        }"#
    }

    fn example_json_with_id(id: &str) -> String {
        example_json().replace("\"id\": \"R4R2-S001\"", &format!("\"id\": \"{id}\""))
    }

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }
}
