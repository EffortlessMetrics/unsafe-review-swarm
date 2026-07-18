//! Analysis-run identity for freshness-aware consumers.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Lifecycle state produced by the analysis-identity slice.
///
/// These are freshness signals only: they describe the relationship between an
/// output and the source it was computed from (current, in-flight, stale,
/// partial, capped, or failed). No variant is a safety or correctness claim
/// beyond this freshness relationship — `Current` does not mean the file is
/// safe, and none of `Refreshing`/`Stale`/`Partial`/`Capped`/`Failed` mean it is
/// unsafe. Consumers must not read any state as proof, UB-free status, or
/// Miri-clean status.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisState {
    /// The output reflects the current source; no known staleness.
    Current,
    /// A newer analysis is already in flight; this output may be superseded shortly.
    Refreshing,
    /// The source has changed since this output was produced.
    Stale,
    /// The analysis completed but covered less than the full requested scope.
    Partial,
    /// The analysis stopped early after hitting a configured limit (e.g. max cards).
    Capped,
    /// The analysis did not complete; this output should not be treated as current.
    Failed,
}

impl AnalysisState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Refreshing => "refreshing",
            Self::Stale => "stale",
            Self::Partial => "partial",
            Self::Capped => "capped",
            Self::Failed => "failed",
        }
    }
}

/// Facts identifying the analysis run that produced an output envelope.
///
/// This is a freshness signal only. Optional source facts remain absent until
/// their actual producer is available; no currentness or safety is implied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisIdentity {
    pub analysis_id: String,
    pub generation: u64,
    pub tool_version: String,
    pub scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub document_version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_digest: Option<String>,
    pub state: AnalysisState,
}

impl AnalysisIdentity {
    /// Create a fresh per-analysis identity.
    pub fn new(scope: impl Into<String>) -> Self {
        let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed) + 1;
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let process = std::process::id();
        Self {
            analysis_id: format!("analysis-{process}-{generation}-{nonce}"),
            generation,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            scope: scope.into(),
            base_commit: None,
            head_commit: None,
            document_version: None,
            file_digest: None,
            state: AnalysisState::Current,
        }
    }

    /// Build a deterministic identity for projection tests.
    pub fn for_test(
        generation: u64,
        analysis_id: impl Into<String>,
        scope: impl Into<String>,
    ) -> Self {
        Self {
            analysis_id: analysis_id.into(),
            generation,
            tool_version: env!("CARGO_PKG_VERSION").to_string(),
            scope: scope.into(),
            base_commit: None,
            head_commit: None,
            document_version: None,
            file_digest: None,
            state: AnalysisState::Current,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AnalysisIdentity, AnalysisState};
    use std::collections::BTreeSet;

    #[test]
    fn every_state_serializes_to_a_distinct_lowercase_string() -> Result<(), String> {
        let variants = [
            AnalysisState::Current,
            AnalysisState::Refreshing,
            AnalysisState::Stale,
            AnalysisState::Partial,
            AnalysisState::Capped,
            AnalysisState::Failed,
        ];
        let mut serialized = Vec::new();
        for variant in &variants {
            let value = serde_json::to_value(variant).map_err(|err| err.to_string())?;
            let text = value
                .as_str()
                .ok_or_else(|| "state should serialize to a JSON string".to_string())?
                .to_string();
            assert_eq!(text, variant.as_str());
            assert_eq!(text, text.to_lowercase());
            serialized.push(text);
        }
        let unique: BTreeSet<_> = serialized.iter().cloned().collect();
        assert_eq!(
            unique.len(),
            variants.len(),
            "every AnalysisState variant must serialize to a distinct string"
        );
        Ok(())
    }

    #[test]
    fn non_current_states_are_distinct_from_current() -> Result<(), String> {
        let non_current = [
            AnalysisState::Failed,
            AnalysisState::Partial,
            AnalysisState::Capped,
            AnalysisState::Stale,
        ];
        for state in non_current {
            assert_ne!(state, AnalysisState::Current);
            let value = serde_json::to_value(&state).map_err(|err| err.to_string())?;
            assert_ne!(
                value.as_str(),
                Some("current"),
                "{state:?} must not serialize the same as Current"
            );
        }
        Ok(())
    }

    #[test]
    fn distinct_runs_do_not_reuse_identity_or_generation() {
        let first = AnalysisIdentity::new("diff");
        let second = AnalysisIdentity::new("diff");
        assert_ne!(first.analysis_id, second.analysis_id);
        assert_ne!(first.generation, second.generation);
    }

    #[test]
    fn test_identity_omits_unknown_source_facts() -> Result<(), String> {
        let identity = AnalysisIdentity::for_test(7, "test-analysis", "diff");
        let value = serde_json::to_value(identity).map_err(|err| err.to_string())?;
        assert!(value.get("base_commit").is_none());
        assert!(value.get("head_commit").is_none());
        assert!(value.get("document_version").is_none());
        assert!(value.get("file_digest").is_none());
        Ok(())
    }
}
