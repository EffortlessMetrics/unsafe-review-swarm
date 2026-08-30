#![forbid(unsafe_code)]

//! `check-pr` component dispatch for `check-local`.
//!
//! This module owns the mapping from `check_local::CATALOG` ids to the
//! concrete `check-pr` component functions, and the execution seam that
//! `check-local` uses to run a single component (including the quiet-mode
//! subprocess path). Keeping recognition (`dispatch_check`) separate from
//! execution (`run_named_check`) lets tests guard against `CATALOG`/dispatch
//! drift without executing the heavyweight checks.

use std::process::{Command, Stdio};

/// Map a `check_local::CATALOG` id to the `check-pr` component that runs it.
///
/// Returns `None` for an unrecognized id. Splitting recognition (this function)
/// from execution ([`run_named_check`]) lets a test assert every catalog id has
/// a dispatch arm without executing the heavyweight checks — the guard against
/// `CATALOG`/dispatch drift. Keep this arm-for-arm in sync with the `CheckPr`
/// match arm in `main.rs` and `check_local::CATALOG`.
pub(crate) fn dispatch_check(id: &str) -> Option<fn() -> Result<(), String>> {
    let check: fn() -> Result<(), String> = match id {
        "docs" => crate::check_docs,
        "generated-projection" => crate::public_badges::check_generated_projection,
        "policy" => crate::check_policy,
        "support-tiers" => crate::support_tiers::check_support_tiers,
        "fixtures" => crate::fixture_surfaces::check_fixtures,
        "calibration" => crate::check_calibration,
        "fixture-surface-parity" => crate::fixture_surfaces::check_fixture_surface_parity,
        "surface-determinism" => crate::fixture_surfaces::check_surface_determinism,
        "real-pr-corpus" => crate::real_pr_corpus::check,
        "corpus-partitions" => crate::corpus_partitions::check,
        "evidence-loss-challenges" => crate::evidence_loss_challenges::check,
        "external-pilots" => crate::external_pilots::check,
        "dogfood" => crate::check_dogfood,
        "fuzz-manual-harness" => crate::fuzz_artifact_checks::check_manual_fuzz_harness,
        "fuzz-tracked-artifacts" => crate::fuzz_artifact_checks::check_tracked_generated_artifacts,
        "self-unsafe" => crate::self_unsafe::check_self_unsafe,
        _ => return None,
    };
    Some(check)
}

/// Execute a single `check-pr` component by its `check_local::CATALOG` id.
///
/// This is the execution seam for `check-local`: the selection logic lives in
/// `check_local` (pure, unit-tested) and this dispatch maps each catalog id to
/// the same function `check-pr` invokes, so the two can never drift in behavior.
pub(crate) fn run_named_check(id: &str, quiet: bool) -> Result<(), String> {
    if quiet {
        let executable = std::env::current_exe()
            .map_err(|err| format!("check-local: failed to resolve xtask executable: {err}"))?;
        let output = Command::new(&executable)
            .arg("check-local-run")
            .arg(id)
            .stdout(Stdio::null())
            .output()
            .map_err(|err| {
                format!(
                    "check-local: failed to run `{id}` quietly via `{}`: {err}",
                    executable.display()
                )
            })?;
        if output.status.success() {
            return Ok(());
        }
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!("check-local: `{id}` exited with {}", output.status)
        } else {
            detail
        });
    }
    match dispatch_check(id) {
        Some(check) => check(),
        None => Err(format!("check-local: unknown check id `{id}`")),
    }
}

#[cfg(test)]
mod tests {
    use super::{dispatch_check, run_named_check};
    use crate::check_local;

    #[test]
    fn every_check_local_catalog_id_has_a_dispatch_arm() {
        // Guards against `check_local::CATALOG` and `dispatch_check` drifting:
        // every catalog id must resolve to a dispatch arm (no execution needed).
        for spec in check_local::CATALOG {
            assert!(
                dispatch_check(spec.id).is_some(),
                "no dispatch arm for check-local catalog id `{}`",
                spec.id
            );
        }
    }

    #[test]
    fn dispatch_check_rejects_unknown_id() {
        assert!(dispatch_check("not-a-real-check").is_none());
    }

    #[test]
    #[allow(clippy::panic, reason = "test assertion uses panic for unreachable OK branch")]
    fn run_named_check_rejects_unknown_id() {
        let err = match run_named_check("not-a-real-check", false) {
            Ok(()) => panic!("run_named_check should reject unknown id"),
            Err(err) => err,
        };
        assert!(err.contains("unknown check id"));
        assert!(err.contains("not-a-real-check"));
    }
}
