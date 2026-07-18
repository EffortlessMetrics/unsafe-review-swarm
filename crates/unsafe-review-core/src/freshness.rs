//! Analysis-run identity for freshness-aware consumers.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Lifecycle state produced by the analysis-identity slice.
///
/// Later #1908 slices add explicit stale, failed, partial, and capped states.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AnalysisState {
    Current,
}

impl AnalysisState {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Current => "current",
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
    use super::AnalysisIdentity;

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
