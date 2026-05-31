use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const MANUAL_CANDIDATE_SCHEMA_VERSION: &str = "manual-candidate/v1";
const MANUAL_CANDIDATE_TRUST_BOUNDARY: &str =
    "manual candidate; not analyzer-discovered; not proof of repository safety";

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
    #[serde(default)]
    pub evidence: Vec<ManualCandidateEvidence>,
    pub trust_boundary: String,
    #[serde(default = "manual_source")]
    pub source: String,
    #[serde(default)]
    pub manual_candidate: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCandidateLocation {
    pub file: PathBuf,
    pub line: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManualCandidateEvidence {
    pub kind: String,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub summary: Option<String>,
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
        require_nonempty("id", &self.id)?;
        if self.id.contains('/') || self.id.contains('\\') {
            return Err("manual candidate id must not contain path separators".to_string());
        }
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
        require_nonempty("trust_boundary", &self.trust_boundary)?;
        for evidence in &self.evidence {
            if !is_known_evidence_kind(&evidence.kind) {
                return Err(format!(
                    "manual candidate evidence kind `{}` is not supported",
                    evidence.kind
                ));
            }
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
    render_candidate_evidence_markdown(&mut out, candidate);
    out.push_str("## Next action\n\n");
    out.push_str("Review the manual candidate, preserve the external evidence packet, and import receipts only when they match this manual candidate ID.\n\n");
    out.push_str("## Trust boundary\n\n");
    out.push_str(&candidate.trust_boundary);
    out.push('\n');
    out
}

pub fn render_manual_candidate_context(candidate: &ManualCandidate) -> Result<String, String> {
    let value = serde_json::json!({
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
        "evidence": candidate.evidence.iter().map(|evidence| serde_json::json!({
            "kind": evidence.kind,
            "path": evidence.path.as_ref().map(|path| path.display().to_string()),
            "summary": evidence.summary,
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
        out.push('\n');
    }
    out.push('\n');
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

fn validate_candidate_id(id: &str) -> Result<(), String> {
    require_nonempty("id", id)?;
    if id.contains('/') || id.contains('\\') {
        return Err("manual candidate id must not contain path separators".to_string());
    }
    Ok(())
}

fn is_known_evidence_kind(kind: &str) -> bool {
    matches!(
        kind,
        "runtime_witness" | "model" | "source_trace" | "node_parity" | "human_review" | "other"
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
        assert_eq!(candidate.evidence.len(), 2);
        assert!(
            candidate
                .to_pretty_json()?
                .contains("\"manual_candidate\": true")
        );
        Ok(())
    }

    #[test]
    fn manual_candidate_rejects_wrong_schema() {
        let err = ManualCandidate::from_json_str(
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
        )
        .unwrap_err();

        assert!(err.contains("schema_version"));
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
    fn manual_candidate_explain_context_and_witness_plan_preserve_manual_marker()
    -> Result<(), String> {
        let candidate = ManualCandidate::from_json_str(example_json())?;

        let explain = render_manual_candidate_explain(&candidate);
        let context = render_manual_candidate_context(&candidate)?;
        let witness_plan = render_manual_candidate_witness_plan(&candidate);

        assert!(explain.contains("Source: `manual`"));
        assert!(explain.contains("Analyzer-discovered: `false`"));
        assert!(context.contains("\"source\": \"manual\""));
        assert!(context.contains("\"manual_candidate\": true"));
        assert!(witness_plan.contains("manual candidate witness plan"));
        assert!(witness_plan.contains("does not run witnesses"));
        Ok(())
    }

    fn example_json() -> &'static str {
        r#"{
          "schema_version": "manual-candidate/v1",
          "id": "R4R2-S001",
          "title": "TextDecoder SharedArrayBuffer decode creates &[u8] over shared bytes",
          "location": {
            "file": "src/runtime/webcore/TextDecoder.rs",
            "line": 237
          },
          "operation_family": "raw_pointer_read",
          "unsafe_operation": "core::slice::from_raw_parts",
          "invariant": "&[u8] memory must not be concurrently mutated",
          "safe_caller": "new TextDecoder().decode(new Uint8Array(new SharedArrayBuffer(...)))",
          "evidence": [
            {
              "kind": "runtime_witness",
              "path": "target/unsafe-scout/textdecoder-shared-race-route.out"
            },
            {
              "kind": "model",
              "path": "target/unsafe-scout/miri-textdecoder-shared-slice.out"
            }
          ],
          "trust_boundary": "manual candidate; not analyzer-discovered; not proof of repository safety"
        }"#
    }

    fn unique_temp_dir(prefix: &str) -> Result<PathBuf, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock before UNIX_EPOCH: {err}"))?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!("{prefix}-{nanos}")))
    }
}
