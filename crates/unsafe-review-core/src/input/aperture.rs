//! Additive per-analysis aperture manifest (issue #2331 PR1).
//!
//! A quiet result can be misread as a strong claim. The [`AnalysisAperture`]
//! states what one analysis actually covered: which files were selected and
//! analyzed, which were unresolved or rejected, what the detector counted,
//! which configuration envelope applied, which caps truncated the scan, and
//! which limitations survive. It references existing canonical facts
//! ([`ChangeSet`](crate::input::changeset::ChangeSet) scope data,
//! [`AnalysisEnvironment`](crate::input::environment::AnalysisEnvironment),
//! [`Summary`](crate::AnalyzeOutput), PR2a configuration counts) without
//! changing detectors, filtering, ranking, or zero-card semantics.
//!
//! The manifest is not a safety score, a precision/recall claim, or a
//! substitute for independent evaluation.

use std::collections::BTreeSet;
use std::path::PathBuf;

/// Version of the aperture manifest schema. Bump when fields change meaning.
pub const APERTURE_SCHEMA_VERSION: u32 = 1;

/// Maximum unresolved/rejected paths listed in human output; JSON always
/// carries the full sorted list and the human section names the remainder.
const HUMAN_PATH_LIMIT: usize = 10;

/// File populations for one analysis revision.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApertureFiles {
    pub changed_files: usize,
    pub changed_rust_files: usize,
    pub rust_files: usize,
    pub analyzed_diff_files: usize,
    pub unresolved_diff_files: Vec<PathBuf>,
    pub rejected_diff_files: Vec<PathBuf>,
}

/// Configuration-envelope coverage for one analysis revision.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApertureConfiguration {
    pub envelope_selected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default)]
    pub gated_cards: usize,
    #[serde(default)]
    pub active: usize,
    #[serde(default)]
    pub inactive: usize,
    #[serde(default)]
    pub unknown: usize,
    #[serde(default)]
    pub unsupported: usize,
    #[serde(default)]
    pub parse_failed: usize,
    #[serde(default)]
    pub parent_unavailable: usize,
    #[serde(default)]
    pub macro_unavailable: usize,
}

/// Cap state for one analysis revision, projected from the pipeline's own
/// cap decision, never re-derived from counts.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApertureCaps {
    pub scan_capped: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_cap: Option<usize>,
}

/// Source-role buckets over emitted cards.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApertureRoles {
    pub production: usize,
    pub test: usize,
    pub generated: usize,
    pub unknown: usize,
}

/// Detector populations the pipeline counts but no card resolves.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ApertureUnsupported {
    pub miri_unsupported: usize,
    pub static_unknown: usize,
}

/// One canonical per-analysis aperture manifest.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AnalysisAperture {
    pub schema_version: u32,
    pub analysis_id: String,
    pub scope: String,
    pub mode: String,
    pub policy: String,
    pub files: ApertureFiles,
    pub unsafe_sites: usize,
    pub cards: usize,
    pub roles: ApertureRoles,
    pub unsupported: ApertureUnsupported,
    pub configuration: ApertureConfiguration,
    pub caps: ApertureCaps,
    pub limitations: Vec<String>,
    pub digest: String,
}

/// Configuration input for [`assemble_aperture`]: the evaluated envelope
/// digest, an optional discovery note, and the evaluated card counts.
/// `None` means no envelope was selected for this run.
#[derive(Clone, Debug)]
pub struct ApertureConfigurationInput<'a> {
    pub environment_digest: &'a str,
    pub note: Option<&'a str>,
    pub counts: crate::input::cfg::ConfigurationCounts,
}

fn sorted_paths(paths: &BTreeSet<PathBuf>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = paths.iter().cloned().collect();
    out.sort();
    out
}

/// Assemble the aperture manifest from one analysis output and an optional
/// evaluated configuration envelope. Deterministic: file lists are sorted
/// and the digest covers the canonical serialization.
pub fn assemble_aperture(
    output: &crate::AnalyzeOutput,
    configuration: Option<ApertureConfigurationInput<'_>>,
) -> AnalysisAperture {
    let summary = &output.summary;
    let configuration = match configuration {
        Some(input) => ApertureConfiguration {
            envelope_selected: true,
            environment_digest: Some(input.environment_digest.to_string()),
            note: input.note.map(str::to_string),
            gated_cards: input.counts.gated,
            active: input.counts.active,
            inactive: input.counts.inactive,
            unknown: input.counts.unknown,
            unsupported: input.counts.unsupported,
            parse_failed: input.counts.parse_failed,
            parent_unavailable: input.counts.parent_unavailable,
            macro_unavailable: input.counts.macro_unavailable,
        },
        None => ApertureConfiguration {
            envelope_selected: false,
            environment_digest: None,
            note: None,
            gated_cards: 0,
            active: 0,
            inactive: 0,
            unknown: 0,
            unsupported: 0,
            parse_failed: 0,
            parent_unavailable: 0,
            macro_unavailable: 0,
        },
    };
    let mut limitations = Vec::new();
    if !output.unresolved_diff_files.is_empty() {
        limitations.push(format!(
            "{} changed paths were unresolved from the analysis root",
            output.unresolved_diff_files.len()
        ));
    }
    if !output.rejected_diff_files.is_empty() {
        limitations.push(format!(
            "{} changed paths were rejected (absolute, traversal, or symlink-escaping)",
            output.rejected_diff_files.len()
        ));
    }
    if let Some(note) = configuration.note.as_deref() {
        limitations.push(note.to_string());
    }
    if summary.scan_capped {
        limitations.push(
            summary
                .capped_scan_notice()
                .unwrap_or_else(|| "scan capped by card cap".to_string()),
        );
    }
    limitations.push(
        "parent-file module gates and macro expansion are not resolved in this revision"
            .to_string(),
    );
    let mut aperture = AnalysisAperture {
        schema_version: APERTURE_SCHEMA_VERSION,
        analysis_id: output.analysis_identity.analysis_id.clone(),
        scope: output.scope.as_str().to_string(),
        mode: output.mode.as_str().to_string(),
        policy: output.policy.as_str().to_string(),
        files: ApertureFiles {
            changed_files: summary.changed_files,
            changed_rust_files: summary.changed_rust_files,
            rust_files: summary.rust_files,
            analyzed_diff_files: output.diff_scoped_files.len(),
            unresolved_diff_files: sorted_paths(&output.unresolved_diff_files),
            rejected_diff_files: sorted_paths(&output.rejected_diff_files),
        },
        unsafe_sites: summary.unsafe_sites,
        cards: summary.cards,
        roles: ApertureRoles {
            production: summary.production_cards,
            test: summary.test_cards,
            generated: summary.generated_cards,
            unknown: summary.unknown_cards,
        },
        unsupported: ApertureUnsupported {
            miri_unsupported: summary.miri_unsupported,
            static_unknown: summary.static_unknown,
        },
        configuration,
        caps: ApertureCaps {
            scan_capped: summary.scan_capped,
            card_cap: summary.card_cap,
        },
        limitations,
        digest: String::new(),
    };
    let encoding = serde_json::to_string(&aperture).unwrap_or_default();
    aperture.digest = format!(
        "aperture-sha256:{}",
        crate::sha256_hex_of(encoding.as_bytes())
    );
    aperture
}

fn render_paths(label: &str, paths: &[PathBuf], out: &mut String) {
    if paths.is_empty() {
        out.push_str(&format!("- {label}: none\n"));
        return;
    }
    out.push_str(&format!("- {label}: {}\n", paths.len()));
    for path in paths.iter().take(HUMAN_PATH_LIMIT) {
        out.push_str(&format!("  - {}\n", path.display()));
    }
    if paths.len() > HUMAN_PATH_LIMIT {
        out.push_str(&format!(
            "  - ... and {} more\n",
            paths.len() - HUMAN_PATH_LIMIT
        ));
    }
}

/// Compact human aperture section: coverage boundary first, full manifest
/// stays in JSON. Rendered only for explicit `--aperture` runs so default
/// output stays byte-stable.
pub fn render_aperture_human(aperture: &AnalysisAperture) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Aperture (analysis {}, {}):\n",
        aperture.analysis_id, aperture.digest
    ));
    out.push_str(&format!(
        "- scope {} / mode {} / policy {}\n",
        aperture.scope, aperture.mode, aperture.policy
    ));
    out.push_str(&format!(
        "- files: {} changed ({} rust) of {} rust files scanned; {} analyzed diff files\n",
        aperture.files.changed_files,
        aperture.files.changed_rust_files,
        aperture.files.rust_files,
        aperture.files.analyzed_diff_files
    ));
    render_paths(
        "unresolved",
        &aperture.files.unresolved_diff_files,
        &mut out,
    );
    render_paths("rejected", &aperture.files.rejected_diff_files, &mut out);
    out.push_str(&format!(
        "- sites: {} unsafe sites, {} cards\n",
        aperture.unsafe_sites, aperture.cards
    ));
    out.push_str(&format!(
        "- roles: {} production, {} test, {} generated, {} unknown\n",
        aperture.roles.production,
        aperture.roles.test,
        aperture.roles.generated,
        aperture.roles.unknown
    ));
    out.push_str(&format!(
        "- unsupported: {} miri-unsupported, {} static-unknown\n",
        aperture.unsupported.miri_unsupported, aperture.unsupported.static_unknown
    ));
    if aperture.configuration.envelope_selected {
        out.push_str(&format!(
            "- configuration: {} gated cards ({} active, {} inactive, {} unknown, {} unsupported, {} parse failed, {} parent unavailable, {} macro unavailable)\n",
            aperture.configuration.gated_cards,
            aperture.configuration.active,
            aperture.configuration.inactive,
            aperture.configuration.unknown,
            aperture.configuration.unsupported,
            aperture.configuration.parse_failed,
            aperture.configuration.parent_unavailable,
            aperture.configuration.macro_unavailable
        ));
    } else {
        out.push_str("- configuration: no envelope selected; cfg applicability not evaluated\n");
    }
    if aperture.caps.scan_capped {
        out.push_str(&format!(
            "- capped: scan capped (card cap {:?})\n",
            aperture.caps.card_cap
        ));
    } else {
        out.push_str("- capped: none\n");
    }
    if aperture.limitations.is_empty() {
        out.push_str("- limitations: none\n");
    } else {
        out.push_str("- limitations:\n");
        for limitation in &aperture.limitations {
            out.push_str(&format!("  - {limitation}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyzed_output() -> crate::AnalyzeOutput {
        crate::AnalyzeOutput {
            analysis_identity: crate::AnalysisIdentity::new("diff"),
            schema_version: "0.1".to_string(),
            tool: "unsafe-review".to_string(),
            root: PathBuf::from("."),
            scope: crate::Scope::Diff,
            mode: crate::AnalysisMode::Draft,
            policy: crate::PolicyMode::Advisory,
            summary: crate::api::Summary::default(),
            cards: Vec::new(),
            diff_scoped_files: BTreeSet::from([PathBuf::from("src/lib.rs")]),
            unresolved_diff_files: BTreeSet::from([PathBuf::from("gone.rs")]),
            rejected_diff_files: BTreeSet::new(),
            coverage_snapshot: std::collections::BTreeMap::new(),
        }
    }

    #[test]
    fn unselected_envelope_reports_no_configuration() {
        let aperture = assemble_aperture(&analyzed_output(), None);
        assert_eq!(aperture.schema_version, APERTURE_SCHEMA_VERSION);
        assert!(!aperture.configuration.envelope_selected);
        assert_eq!(aperture.files.unresolved_diff_files.len(), 1);
        assert!(aperture.digest.starts_with("aperture-sha256:"));
        let human = render_aperture_human(&aperture);
        assert!(human.contains("no envelope selected"));
    }

    #[test]
    fn manifest_is_deterministic_for_the_same_input() {
        let output = analyzed_output();
        let first = assemble_aperture(&output, None);
        let second = assemble_aperture(&output, None);
        assert_eq!(first.digest, second.digest);
        assert_eq!(first, second);
    }

    #[test]
    fn capped_scans_name_the_cap_limitation() {
        let mut output = analyzed_output();
        output.summary.scan_capped = true;
        output.summary.card_cap = Some(25);
        let aperture = assemble_aperture(&output, None);
        assert!(aperture.caps.scan_capped);
        assert!(
            aperture
                .limitations
                .iter()
                .any(|limitation| limitation.contains("25"))
        );
    }

    #[test]
    fn evaluated_envelope_counts_survive_assembly() -> Result<(), String> {
        let output = analyzed_output();
        let counts = crate::input::cfg::ConfigurationCounts {
            gated: 4,
            active: 2,
            inactive: 1,
            unknown: 1,
            ..crate::input::cfg::ConfigurationCounts::default()
        };
        let aperture = assemble_aperture(
            &output,
            Some(ApertureConfigurationInput {
                environment_digest: "environment-sha256:abc",
                note: None,
                counts,
            }),
        );
        if !aperture.configuration.envelope_selected {
            return Err("envelope must be selected".to_string());
        }
        if aperture.configuration.gated_cards != 4 || aperture.configuration.active != 2 {
            return Err("counts must survive assembly".to_string());
        }
        Ok(())
    }
}
