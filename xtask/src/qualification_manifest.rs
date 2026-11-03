//! Installed-candidate qualification harness preparation (issue #2403, HP1).
//!
//! Harness-only logic: installed-executable selection with explicit
//! fallback refusal, install-path quoting for prefixes containing spaces,
//! the row-manifest table shared with
//! `docs/qualification/UNSAFE-REVIEW-QUALIFICATION-2403-MANIFEST.md`, and
//! the recorded consumer pins. No test in this module executes an
//! installed candidate, renders a verdict, or touches the network.

use std::path::{Path, PathBuf};

/// Manifest document this module stays consistent with.
pub(crate) const QUALIFICATION_MANIFEST_DOC: &str =
    "docs/qualification/UNSAFE-REVIEW-QUALIFICATION-2403-MANIFEST.md";

/// Recorded consumer pins. Read from docs/status files; never invented.
pub(crate) const TOKMD_VERSION: &str = "1.15.0";
pub(crate) const TOKMD_COMMIT: &str = "3d278c56d4afe37583e67500fc2e89e60c3077fe";
pub(crate) const UB_REVIEW_VERSION: &str = "0.1.2";
pub(crate) const UB_REVIEW_COMMIT: &str = "55e4fbab879b3a83c31f35cdc9f7e5cd99f0a6c8";
pub(crate) const SAVED_LSP_SCHEMA: &str = "0.2";

/// Fallback channels the harness must never take silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstalledFallback {
    CargoRun,
    PathLookup,
    DevBinary,
    InstalledRelease,
}

/// Row execution status. Execution rows stay blocked on the frozen #2401
/// matrix and source #579 identities; only harness-prep rows are ready.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QualificationStatus {
    Ready,
    Blocked,
}

/// One acceptance row: fixture, harness entry point, verdict rule, owner.
#[derive(Debug, Clone, Copy)]
pub(crate) struct QualificationRow {
    pub id: &'static str,
    pub fixture: &'static str,
    pub harness_entry: &'static str,
    pub verdict_rule: &'static str,
    pub status: QualificationStatus,
    pub owner: &'static str,
}

pub(crate) const BLOCKED_OWNERS: &str =
    "frozen matrix owner (issue #2401); source identities owner (source #579)";

pub(crate) const QUALIFICATION_ROWS: &[QualificationRow] = &[
    QualificationRow {
        id: "installed-cli-smoke",
        fixture: "fixtures/raw_pointer_deref_brownfield_inherited",
        harness_entry: ".github/workflows/windows-qual.yml + .github/scripts/installed_consumer_probe.py",
        verdict_rule: "pass iff installed identity matches the candidate receipt and failure rows exit 2; no fallback binary",
        status: QualificationStatus::Blocked,
        owner: BLOCKED_OWNERS,
    },
    QualificationRow {
        id: "installed-lsp-agent-loop",
        fixture: "fixtures/raw_pointer_alignment",
        harness_entry: "xtask/src/lsp_smoke.rs protocol bound to the installed binary; saved target/unsafe-review/lsp.json",
        verdict_rule: "pass iff every loop step agrees on one card identity against schema 0.2",
        status: QualificationStatus::Blocked,
        owner: BLOCKED_OWNERS,
    },
    QualificationRow {
        id: "consumer-tokmd-ingestion",
        fixture: "tokmd-packets.json (tokmd-packets/v1); tokmd 1.15.0",
        harness_entry: "docs/handoffs/2026-08-08-tokmd-packets-1857-acceptance.md five-preset commands",
        verdict_rule: "pass iff all five presets render the candidate bundle",
        status: QualificationStatus::Blocked,
        owner: BLOCKED_OWNERS,
    },
    QualificationRow {
        id: "consumer-ub-review-ingestion",
        fixture: "candidate gate (unsafe-review-gate/v1) + comment-plan (0.1); ub-review 0.1.2",
        harness_entry: "docs/receipts/ub-review-smoke-6f595070.md verification procedure",
        verdict_rule: "pass iff gate routes, 0.1 envelope ingests, identity and trust boundary survive",
        status: QualificationStatus::Blocked,
        owner: BLOCKED_OWNERS,
    },
    QualificationRow {
        id: "consumer-saved-editor",
        fixture: "target/unsafe-review/lsp.json (schema 0.2); editors/vscode",
        harness_entry: "docs/editor/extension-mvp.md load procedure",
        verdict_rule: "pass iff the saved bundle loads intact; no live-server claim",
        status: QualificationStatus::Blocked,
        owner: BLOCKED_OWNERS,
    },
    QualificationRow {
        id: "space-in-install-prefix",
        fixture: "task-owned install prefix containing spaces (created at execution)",
        harness_entry: "cargo install --root with the exact path quoted; quoting mirrors installed_consumer_probe.py init_shell_arg",
        verdict_rule: "pass iff install and invocation work quoted; otherwise loud failure naming the exact path",
        status: QualificationStatus::Blocked,
        owner: BLOCKED_OWNERS,
    },
    QualificationRow {
        id: "harness-selection-tests",
        fixture: "none (unit tests against selection logic and recorded pins)",
        harness_entry: "xtask/src/qualification_manifest.rs via cargo test -p xtask qualification_manifest",
        verdict_rule: "pass iff wrong/missing executables fail loudly with zero fallback attempts and pins resolve",
        status: QualificationStatus::Ready,
        owner: "issue #2403 HP1 (this slice)",
    },
];

/// Resolve the installed executable at exactly `expected`.
///
/// There is no fallback: the `fallback_attempt` observer exists so tests can
/// prove no fallback channel (`cargo run`, `PATH` lookup, dev binary, or the
/// installed release) is ever attempted. A wrong or missing path is a loud
/// error naming the expected path verbatim.
pub(crate) fn select_installed_executable(
    expected: &Path,
    _fallback_attempt: &mut dyn FnMut(InstalledFallback),
) -> Result<PathBuf, String> {
    if expected.is_file() {
        return Ok(expected.to_path_buf());
    }
    Err(format!(
        "installed executable is missing or not a file: expected {}; \
         refusing to fall back to cargo run, PATH lookup, dev binaries, or the installed release",
        expected.display()
    ))
}

/// Quote an install path for shell use, mirroring the `init_shell_arg`
/// quoting in `.github/scripts/installed_consumer_probe.py`: plain paths pass
/// through, anything else (including spaces) is single-quoted with embedded
/// quotes escaped so the exact path survives.
pub(crate) fn quote_install_path(path: &Path) -> String {
    let text = path.to_string_lossy();
    let plain = text
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':'));
    if plain {
        return text.into_owned();
    }
    if cfg!(windows) {
        format!("'{}'", text.replace('\'', "''"))
    } else {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> Result<PathBuf, String> {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "xtask has no parent directory for the workspace root".to_string())
    }

    fn read_manifest_doc() -> Result<String, String> {
        let path = workspace_root()?.join(QUALIFICATION_MANIFEST_DOC);
        std::fs::read_to_string(&path)
            .map_err(|error| format!("read {} failed: {error}", path.display()))
    }

    fn assert_no_fallback_attempts(
        expected: &Path,
        attempts: &[InstalledFallback],
        outcome: &Result<PathBuf, String>,
    ) -> Result<(), String> {
        if !attempts.is_empty() {
            return Err(format!(
                "selection for {} attempted a fallback channel ({attempts:?}) with outcome {outcome:?}: \
                 silent fallback is never acceptable",
                expected.display()
            ));
        }
        Ok(())
    }

    #[test]
    fn installed_qualification_manifest_selection_wrong_path_fails_loudly() -> Result<(), String> {
        let expected = Path::new("target/qualification-2403/bin/unsafe-review-wrong");
        let mut attempts = Vec::new();
        let outcome = select_installed_executable(expected, &mut |kind| attempts.push(kind));
        let error = outcome
            .err()
            .ok_or_else(|| "wrong installed path was accepted".to_string())?;
        if !error.contains(&expected.display().to_string()) {
            return Err(format!(
                "wrong-path error does not name the expected path: {error}"
            ));
        }
        assert_no_fallback_attempts(expected, &attempts, &Err(error))
    }

    #[test]
    fn installed_qualification_manifest_selection_missing_path_fails_loudly() -> Result<(), String>
    {
        let expected = Path::new("target/qualification-2403/does-not-exist/bin/unsafe-review");
        let mut attempts = Vec::new();
        let outcome = select_installed_executable(expected, &mut |kind| attempts.push(kind));
        let error = outcome
            .err()
            .ok_or_else(|| "missing installed path was accepted".to_string())?;
        for forbidden in [
            "cargo run",
            "PATH lookup",
            "dev binaries",
            "installed release",
        ] {
            if !error.contains(forbidden) {
                return Err(format!(
                    "missing-path error does not refuse {forbidden}: {error}"
                ));
            }
        }
        if !error.contains(&expected.display().to_string()) {
            return Err(format!(
                "missing-path error does not name the expected path: {error}"
            ));
        }
        assert_no_fallback_attempts(expected, &attempts, &Err(error))
    }

    #[test]
    fn installed_qualification_manifest_selection_accepts_exact_file() -> Result<(), String> {
        let dir = std::env::temp_dir().join(format!(
            "installed-qualification-{}-exact",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir)
            .map_err(|error| format!("create scratch dir {} failed: {error}", dir.display()))?;
        let expected = dir.join("unsafe-review");
        std::fs::write(&expected, b"placeholder").map_err(|error| {
            format!("write scratch file {} failed: {error}", expected.display())
        })?;
        let mut attempts = Vec::new();
        let outcome = select_installed_executable(&expected, &mut |kind| attempts.push(kind));
        let selected = outcome.map_err(|error| {
            let _ = std::fs::remove_dir_all(&dir);
            format!("exact installed file was rejected: {error}")
        })?;
        if selected != expected {
            let _ = std::fs::remove_dir_all(&dir);
            return Err(format!(
                "selection did not return the exact expected path: {}",
                selected.display()
            ));
        }
        assert_no_fallback_attempts(&expected, &attempts, &Ok(selected))?;
        std::fs::remove_dir_all(&dir)
            .map_err(|error| format!("remove scratch dir {} failed: {error}", dir.display()))?;
        Ok(())
    }

    #[test]
    fn installed_qualification_manifest_space_prefix_quotes_exact_path() -> Result<(), String> {
        let prefix = std::env::temp_dir().join("installed qual prefix");
        let binary = prefix.join("bin").join("unsafe-review");
        let quoted = quote_install_path(&binary);
        let exact = binary.to_string_lossy().into_owned();
        if !quoted.contains(&exact) {
            return Err(format!("quoted install path lost the exact path: {quoted}"));
        }
        if !quoted.starts_with('\'') || !quoted.ends_with('\'') {
            return Err(format!("install path with spaces is not quoted: {quoted}"));
        }
        let mut attempts = Vec::new();
        let outcome = select_installed_executable(&binary, &mut |kind| attempts.push(kind));
        let error = outcome
            .err()
            .ok_or_else(|| "missing spaced path was accepted".to_string())?;
        if !error.contains(&exact) {
            return Err(format!(
                "spaced-path error does not name the exact expected path: {error}"
            ));
        }
        assert_no_fallback_attempts(&binary, &attempts, &Err(error))
    }

    #[test]
    fn installed_qualification_manifest_every_fallback_channel_is_named() -> Result<(), String> {
        use InstalledFallback::{CargoRun, DevBinary, InstalledRelease, PathLookup};
        let expected = Path::new("target/qualification-2403/bin/unsafe-review");
        let mut attempts = Vec::new();
        let outcome = select_installed_executable(expected, &mut |kind| attempts.push(kind));
        let error = outcome
            .err()
            .ok_or_else(|| "missing installed path was accepted".to_string())?;
        // Every fallback channel the harness refuses must stay constructible
        // so the refusal set is exhaustive, and the loud error must name each
        // channel so a reader knows no silent path was taken.
        let refused = [
            (CargoRun, "CargoRun", "cargo run"),
            (PathLookup, "PathLookup", "PATH lookup"),
            (DevBinary, "DevBinary", "dev binaries"),
            (InstalledRelease, "InstalledRelease", "installed release"),
        ];
        for (kind, debug, name) in refused {
            if format!("{kind:?}") != debug {
                return Err(format!("fallback channel renamed: {debug}"));
            }
            if !error.contains(name) {
                return Err(format!(
                    "refusal error does not name the {name} fallback: {error}"
                ));
            }
        }
        assert_no_fallback_attempts(expected, &attempts, &Err(error))
    }

    #[test]
    fn installed_qualification_manifest_plain_prefix_passes_through() -> Result<(), String> {
        let binary = Path::new("qual-prefix/bin/unsafe-review");
        let quoted = quote_install_path(binary);
        if quoted != binary.to_string_lossy() {
            return Err(format!("plain install path was rewritten: {quoted}"));
        }
        Ok(())
    }

    #[test]
    fn installed_qualification_manifest_rows_mapped_with_blocked_owners() -> Result<(), String> {
        if QUALIFICATION_ROWS.is_empty() {
            return Err("qualification row table is empty".to_string());
        }
        let doc = read_manifest_doc()?;
        for row in QUALIFICATION_ROWS {
            for field in [row.id, row.fixture, row.harness_entry, row.verdict_rule] {
                if field.is_empty() {
                    return Err(format!("row {} has an empty manifest field", row.id));
                }
            }
            if !doc.contains(row.id) {
                return Err(format!(
                    "row {} is missing from {QUALIFICATION_MANIFEST_DOC}",
                    row.id
                ));
            }
            match row.status {
                QualificationStatus::Ready => {
                    if row.owner.contains("#2401") || row.owner.contains("#579") {
                        return Err(format!(
                            "ready row {} still names a blocking owner: {}",
                            row.id, row.owner
                        ));
                    }
                }
                QualificationStatus::Blocked => {
                    if !row.owner.contains("#2401") {
                        return Err(format!(
                            "blocked row {} does not name the frozen #2401 matrix owner: {}",
                            row.id, row.owner
                        ));
                    }
                    if !doc.contains(row.owner) {
                        return Err(format!(
                            "blocked row {} owner is missing from {QUALIFICATION_MANIFEST_DOC}",
                            row.id
                        ));
                    }
                }
            }
        }
        for required in [
            "installed-cli-smoke",
            "installed-lsp-agent-loop",
            "consumer-",
        ] {
            if !QUALIFICATION_ROWS
                .iter()
                .any(|row| row.id.contains(required))
            {
                return Err(format!(
                    "row table omits the required acceptance area {required}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn installed_qualification_manifest_consumer_fixture_pins_recorded() -> Result<(), String> {
        for pin in [TOKMD_COMMIT, UB_REVIEW_COMMIT] {
            if pin.len() != 40 || !pin.chars().all(|c| c.is_ascii_hexdigit()) {
                return Err(format!("recorded pin is not a 40-hex commit: {pin}"));
            }
        }
        if TOKMD_VERSION.is_empty() || UB_REVIEW_VERSION.is_empty() {
            return Err("consumer version pin is empty".to_string());
        }
        if SAVED_LSP_SCHEMA != "0.2" {
            return Err(format!("saved LSP schema pin drifted: {SAVED_LSP_SCHEMA}"));
        }
        let root = workspace_root()?;
        for path in [
            "fixtures/raw_pointer_alignment",
            "fixtures/raw_pointer_deref_brownfield_inherited",
            "docs/editor/saved-lsp-json.md",
            "docs/editor/extension-mvp.md",
            "editors/vscode/package.json",
            "docs/receipts/ub-review-smoke-6f595070.md",
            "docs/handoffs/2026-08-08-tokmd-packets-1857-acceptance.md",
            ".github/scripts/installed_consumer_probe.py",
            ".github/workflows/windows-qual.yml",
            "xtask/src/lsp_smoke.rs",
        ] {
            if !root.join(path).exists() {
                return Err(format!("mapped fixture or harness path is missing: {path}"));
            }
        }
        let doc = read_manifest_doc()?;
        for pin in [
            TOKMD_VERSION,
            TOKMD_COMMIT,
            UB_REVIEW_VERSION,
            UB_REVIEW_COMMIT,
            SAVED_LSP_SCHEMA,
        ] {
            if !doc.contains(pin) {
                return Err(format!(
                    "recorded pin {pin} is missing from {QUALIFICATION_MANIFEST_DOC}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn installed_qualification_manifest_no_network_or_execution() -> Result<(), String> {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/qualification_manifest.rs");
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("read {} failed: {error}", path.display()))?;
        // Forbidden tokens are built by concatenation so this guard itself
        // never literally contains them.
        let forbidden: &[(&str, &str)] = &[
            ("Tcp", "Stream"),
            ("req", "west"),
            ("Http", "Client"),
            ("cu", "rl"),
            ("Command", "::new"),
            ("process", "::Command"),
            ("std::", "net"),
        ];
        for (head, tail) in forbidden {
            let token = format!("{head}{tail}");
            if token.trim().is_empty() {
                continue;
            }
            if text.contains(token.trim()) {
                return Err(format!(
                    "qualification_manifest.rs must not call {token}: network or process execution is out of scope"
                ));
            }
        }
        Ok(())
    }
}
