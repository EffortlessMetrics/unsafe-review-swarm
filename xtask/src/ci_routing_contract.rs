//! CI routing-contract gate (`check_ci_routing_contract`).
//!
//! Extracted from `main.rs` as part of #1806 (xtask modularization). Validates
//! that `.github/workflows/ci.yml` keeps the single tight-gate CI contract:
//! required markers for the one self-hosted-primary / gh-hosted-overflow gate,
//! and forbidden markers that would reintroduce the retired size-routed
//! multi-lane pile-of-checks. Also validates that the standalone advisory
//! ub-review lane (`.github/workflows/ub-review.yml`) keeps its non-blocking
//! advisory posture: SHA-pinned action, continue-on-error, fail-on-gate off,
//! and fork/draft-guarded secret use.

/// Check a workflow's text against a lane contract: every `required` marker
/// must be present and every `forbidden` marker absent. `lane` names the
/// contract in error messages so a failure reads as a specific stance
/// violation, not a generic string miss.
fn check_lane_markers(
    path: &str,
    text: &str,
    lane: &str,
    required: &[&str],
    forbidden: &[&str],
) -> Result<(), String> {
    for needle in required {
        if !text.contains(needle) {
            return Err(format!("{path} missing required {lane} marker: {needle}"));
        }
    }
    for needle in forbidden {
        if text.contains(needle) {
            return Err(format!(
                "{path} must not carry forbidden {lane} marker: {needle}"
            ));
        }
    }
    Ok(())
}

/// Validate the bounded diagnostic artifact attached to a failed deterministic
/// core gate. The upload may report evidence, but it must neither retain the
/// raw log nor replace the final non-zero verdict with an advisory outcome.
fn check_core_failure_evidence_contract(path: &str, text: &str) -> Result<(), String> {
    check_lane_markers(
        path,
        text,
        "core failure-evidence contract",
        &[
            // The deterministic verdict remains the required floor.
            "test \"${core_exit}\" = \"0\"",
            // This run's verdict cannot be satisfied by stale core_exit state.
            "CORE_RUN_KEY: ${{ github.run_id }}-${{ github.run_attempt }}",
            "core_exit-${CORE_RUN_KEY}",
            "rm -f target/ci-core/core_exit \"$core_exit_path\"",
            "while [ ! -f \"$core_exit_path\" ]",
            "mv \"${core_exit_path}.tmp\" \"$core_exit_path\"",
            // Only closed-vocabulary step status reaches the bounded artifact.
            "step_id\\telapsed_seconds\\texit_status",
            "$1 ~ /^(fmt|clippy|test|check-pr)$/",
            "case \"$core_mode\" in",
            "head -n 80",
            "head -c 16384",
            // Staging is recreated after the core finishes in a private,
            // unpredictable runner-temp path. Only two exact regular files are
            // exposed to upload-artifact via step outputs.
            "id: core-verdict",
            "${RUNNER_TEMP}/unsafe-review-core-evidence-${CORE_RUN_KEY}",
            "rm -rf \"$failure_root\"",
            "mktemp -d \"${failure_root}/staging-XXXXXX\"",
            "[ ! -L \"$failure_summary_path\" ]",
            "[ ! -L \"$failure_metadata_path\" ]",
            "summary_path=$failure_summary_path",
            "metadata_path=$failure_metadata_path",
            // Success clears and never creates a failure artifact directory.
            "rm -rf target/ci-core/failure-evidence",
            "if [ \"${core_exit}\" != \"0\" ]; then",
            // Artifact retention is always attempted but cannot change the job.
            "name: Upload bounded core-gate failure evidence",
            "${{ always() &&",
            "continue-on-error: true",
            "uses: actions/upload-artifact@v7",
            "path: |",
            "${{ steps.core-verdict.outputs.summary_path }}",
            "${{ steps.core-verdict.outputs.metadata_path }}",
            "if-no-files-found: ignore",
            "retention-days: 7",
        ],
        &[
            // Uploading the runner-local raw log would defeat the bound and the
            // explicit redaction step.
            "path: target/ci-core/core.log",
            "path: target/ci-core/failure-evidence/",
            "path: $failure_dir",
            "tail -n 80 target/ci-core/core.log",
            "cat target/ci-core/core.log",
        ],
    )
}

/// Validate the single-gate CI routing contract in `.github/workflows/ci.yml`.
pub(crate) fn check_ci_routing_contract() -> Result<(), String> {
    let path = ".github/workflows/ci.yml";
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("failed to read {path}: {err}"))?;
    // Single tight CI gate, self-hosted-primary with gh-hosted overflow: a minimal
    // `route` job (not a required check) picks the gate runner — an idle trusted
    // self-hosted em-ci runner when the owned fleet has capacity, else
    // `ubuntu-latest` overflow (bursts, capacity gaps, fork PRs). The gate stays a
    // SINGLE job whose mandatory deterministic core floor (`xtask check-pr` plus the
    // full suite) is the only hard blocker and the only required status check. The
    // advisory ub-review LLM lane runs as its own standalone non-blocking workflow
    // (validated below). The router never blocks the merge and never size-routes.
    //
    // The capacity router is allowed ONLY in its minimal self-hosted-primary /
    // gh-overflow shape. The OLD size-routed multi-lane pile-of-checks must not
    // reappear: no per-size lanes (cpx42/cx43/cx53), no separate normalized "Rust
    // Small Result" required check, no budget opt-out fallback modes, and no
    // repository-level runner discovery or the broken Docker Rust Small image.
    check_lane_markers(
        path,
        &text,
        "single-gate CI contract",
        &[
            // One required check, stable name for branch protection.
            "name: Unsafe Review Rust Result",
            // Capacity router: self-hosted primary, gh-hosted overflow.
            "Route CI runner",
            "EM_RUNNER_READ_TOKEN",
            "gh api \"orgs/EffortlessMetrics/actions/runners",
            "runner_kind",
            // Trusted self-hosted label set (shared em-ci group, any idle size).
            "self-hosted",
            "em-ci",
            "trusted-pr",
            // The gate consumes the router's runs-on value; gh-hosted is the overflow.
            "fromJSON(needs.route.outputs.runner)",
            "runs-on: ubuntu-latest",
            // Shared warmed setup, runner-kind agnostic.
            "dtolnay/rust-toolchain@1.95.0",
            "Swatinem/rust-cache@v2",
            // Fast precontext writes the run record, the deterministic core gate
            // runs in the background (guarded by a disk-headroom check), and the
            // final assert decides the merge on the core verdict.
            "Fast precontext and launch core gate",
            "cargo run --locked -p xtask -- check-pr",
            "df -h",
            "core_exit",
            "Assert core gate verdict",
            // The router must stay fork-safe: fork PRs always overflow to gh-hosted.
            "github.event.pull_request.head.repo.fork",
        ],
        &[
            "route-rust-small",
            "router_target=",
            "cpx42",
            "cx43",
            "cx53",
            "Rust Small Fallback on GitHub Hosted",
            "fallback_mode=full",
            "no-github-fallback",
            "Unsafe Review Rust Small Result",
            "em-ci-rust:1.95",
            "docker run --rm",
            // The advisory ub-review lane moved to the standalone ub-review.yml
            // workflow; an in-job copy would double-run (and double-post) the
            // advisory review on every PR.
            "EffortlessMetrics/ub-review@",
        ],
    )?;
    check_core_failure_evidence_contract(path, &text)?;
    if text.contains("repos/${") && text.contains("/actions/runners") {
        return Err(format!(
            "{path} must not reintroduce repository runner discovery (org-level only)"
        ));
    }
    check_ub_review_advisory_contract()
}

/// Validate the standalone advisory ub-review lane contract in
/// `.github/workflows/ub-review.yml`: the action stays SHA-pinned, the job
/// stays non-blocking (continue-on-error, fail-on-gate off, bounded timeout),
/// superseded runs are cancelled, and org-secret use stays guarded to
/// same-repo non-draft PRs. The lane must never become a required check;
/// branch protection names only "Unsafe Review Rust Result" from ci.yml.
fn check_ub_review_advisory_contract() -> Result<(), String> {
    let path = ".github/workflows/ub-review.yml";
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("failed to read {path}: {err}"))?;
    check_lane_markers(
        path,
        &text,
        "advisory ub-review lane",
        &[
            // SHA-pinned advisory action; a re-tag cannot silently change the lane.
            "uses: EffortlessMetrics/ub-review@",
            // Non-blocking advisory posture.
            "continue-on-error: true",
            "fail-on-gate: 'false'",
            "timeout-minutes:",
            // Same-repo guard: fork PRs cannot read the MINIMAX_API_KEY org secret.
            "github.event.pull_request.head.repo.full_name == github.repository",
            // Draft guard + superseded-run cancellation bound advisory LLM cost.
            "github.event.pull_request.draft == false",
            "cancel-in-progress: true",
            // Posts one grouped advisory review; the only reason for
            // pull-requests: write in this workflow (job-scoped).
            "posting: review",
            "pull-requests: write",
        ],
        &[
            // The advisory lane must never gain gate authority or extra tokens.
            "fail-on-gate: 'true'",
            "fail-on-gate: true",
            "checks: write",
            "contents: write",
            "issues: write",
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::check_core_failure_evidence_contract;

    const FAILURE_EVIDENCE_FIXTURE: &str = r#"
CORE_RUN_KEY: ${{ github.run_id }}-${{ github.run_attempt }}
core_exit_path="target/ci-core/core_exit-${CORE_RUN_KEY}"
rm -f target/ci-core/core_exit "$core_exit_path"
rm -rf target/ci-core/failure-evidence
mv "${core_exit_path}.tmp" "$core_exit_path"
while [ ! -f "$core_exit_path" ]; do
  sleep 5
done
test "${core_exit}" = "0"
printf 'step_id\telapsed_seconds\texit_status\n'
$1 ~ /^(fmt|clippy|test|check-pr)$/
case "$core_mode" in
  without-tests|with-tests) ;;
esac
head -n 80
head -c 16384
- name: Assert core gate verdict
  id: core-verdict
if [ "${core_exit}" != "0" ]; then
  failure_root="${RUNNER_TEMP}/unsafe-review-core-evidence-${CORE_RUN_KEY}"
  rm -rf "$failure_root"
  failure_dir="$(mktemp -d "${failure_root}/staging-XXXXXX")"
  failure_summary_path="${failure_dir}/summary.md"
  failure_metadata_path="${failure_dir}/metadata.json"
  if [ -f "$failure_summary_path" ] && [ ! -L "$failure_summary_path" ] \
    && [ -f "$failure_metadata_path" ] && [ ! -L "$failure_metadata_path" ]; then
    echo "summary_path=$failure_summary_path" >> "$GITHUB_OUTPUT"
    echo "metadata_path=$failure_metadata_path" >> "$GITHUB_OUTPUT"
  fi
fi
- name: Upload bounded core-gate failure evidence
  if: >-
    ${{ always() &&
        steps.core-verdict.outputs.summary_path != '' &&
        steps.core-verdict.outputs.metadata_path != '' }}
  continue-on-error: true
  uses: actions/upload-artifact@v7
  with:
    path: |
      ${{ steps.core-verdict.outputs.summary_path }}
      ${{ steps.core-verdict.outputs.metadata_path }}
    if-no-files-found: ignore
    retention-days: 7
"#;

    fn bounded_core_status_summary(input: &str) -> String {
        const MAX_LINES: usize = 80;
        const MAX_BYTES: usize = 16_384;

        let mut output = "step_id\telapsed_seconds\texit_status\n".to_string();
        let mut line_count = 1;
        for line in input.lines() {
            let mut fields = line.split('\t');
            let Some(step_id) = fields.next() else {
                continue;
            };
            let Some(elapsed) = fields.next() else {
                continue;
            };
            let Some(status) = fields.next() else {
                continue;
            };
            if fields.next().is_some()
                || !matches!(step_id, "fmt" | "clippy" | "test" | "check-pr")
                || elapsed.is_empty()
                || elapsed.len() > 10
                || !elapsed.bytes().all(|byte| byte.is_ascii_digit())
                || !valid_core_status(status)
            {
                continue;
            }
            let normalized = format!("{step_id}\t{elapsed}\t{status}\n");
            if line_count >= MAX_LINES || output.len() + normalized.len() > MAX_BYTES {
                break;
            }
            output.push_str(&normalized);
            line_count += 1;
        }
        output
    }

    fn valid_core_status(status: &str) -> bool {
        status == "skipped"
            || (!status.is_empty()
                && status.len() <= 3
                && status.bytes().all(|byte| byte.is_ascii_digit()))
    }

    #[test]
    fn live_ci_workflow_satisfies_failure_evidence_contract() -> Result<(), String> {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows/ci.yml");
        let text = std::fs::read_to_string(&path)
            .map_err(|error| format!("failed to read {}: {error}", path.display()))?;
        check_core_failure_evidence_contract(&path.display().to_string(), &text)
    }

    #[test]
    fn accepts_bounded_failure_evidence_with_required_verdict() -> Result<(), String> {
        check_core_failure_evidence_contract("fixture.yml", FAILURE_EVIDENCE_FIXTURE)
    }

    #[test]
    fn allowlisted_summary_rejects_bare_tokens_and_pem_material() -> Result<(), String> {
        let fixture = "fmt\t1\t0\n\
ghp_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\n\
-----BEGIN PRIVATE KEY-----\n\
check-pr\t2\t101\n\
-----END PRIVATE KEY-----\n\
test\t3\teyJhbGciOiJIUzI1NiJ9.payload.signature\n\
clippy\t4\t0\tbare-secret-material\n";
        let summary = bounded_core_status_summary(fixture);
        let expected = "step_id\telapsed_seconds\texit_status\nfmt\t1\t0\ncheck-pr\t2\t101\n";
        if summary != expected {
            return Err(format!("unexpected closed-vocabulary summary: {summary:?}"));
        }
        for forbidden in ["ghp_", "PRIVATE KEY", "eyJhbGci", "bare-secret-material"] {
            if summary.contains(forbidden) {
                return Err(format!(
                    "summary retained forbidden secret fixture: {forbidden}"
                ));
            }
        }
        Ok(())
    }

    #[test]
    fn allowlisted_summary_enforces_line_and_byte_bounds() -> Result<(), String> {
        let mut fixture = String::new();
        for _ in 0..200 {
            fixture.push_str("check-pr\t1234567890\t101\n");
        }
        fixture.push_str(&"bare-token".repeat(4_096));
        let summary = bounded_core_status_summary(&fixture);
        if summary.lines().count() > 80 {
            return Err(format!(
                "summary exceeded 80-line bound: {}",
                summary.lines().count()
            ));
        }
        if summary.len() > 16_384 {
            return Err(format!(
                "summary exceeded 16-KiB bound: {} bytes",
                summary.len()
            ));
        }
        Ok(())
    }

    #[test]
    fn rejects_failure_evidence_that_weakens_required_verdict() -> Result<(), String> {
        let weakened = FAILURE_EVIDENCE_FIXTURE.replace("test \"${core_exit}\" = \"0\"", "");
        let Err(error) = check_core_failure_evidence_contract("fixture.yml", &weakened) else {
            return Err("weakened core verdict fixture unexpectedly passed".to_string());
        };
        if !error.contains("test \"${core_exit}\" = \"0\"") {
            return Err(format!("unexpected weakened-verdict error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_failure_evidence_without_discoverable_metadata() -> Result<(), String> {
        let missing_metadata =
            FAILURE_EVIDENCE_FIXTURE.replace("${{ steps.core-verdict.outputs.metadata_path }}", "");
        let Err(error) = check_core_failure_evidence_contract("fixture.yml", &missing_metadata)
        else {
            return Err("missing failure metadata fixture unexpectedly passed".to_string());
        };
        if !error.contains("${{ steps.core-verdict.outputs.metadata_path }}") {
            return Err(format!("unexpected missing-metadata error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_contract_that_can_observe_stale_core_exit() -> Result<(), String> {
        let stale = FAILURE_EVIDENCE_FIXTURE
            .replace("rm -f target/ci-core/core_exit \"$core_exit_path\"", "");
        let Err(error) = check_core_failure_evidence_contract("fixture.yml", &stale) else {
            return Err("stale core-exit fixture unexpectedly passed".to_string());
        };
        if !error.contains("rm -f target/ci-core/core_exit") {
            return Err(format!("unexpected stale-exit error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_contract_that_can_upload_failure_evidence_on_success() -> Result<(), String> {
        let stale_artifact =
            FAILURE_EVIDENCE_FIXTURE.replace("rm -rf target/ci-core/failure-evidence", "");
        let Err(error) = check_core_failure_evidence_contract("fixture.yml", &stale_artifact)
        else {
            return Err("success-path stale artifact fixture unexpectedly passed".to_string());
        };
        if !error.contains("rm -rf target/ci-core/failure-evidence") {
            return Err(format!("unexpected stale-artifact error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn explicit_upload_paths_exclude_prepopulated_extra_and_core_log_symlink() -> Result<(), String>
    {
        let hostile_fixture = format!(
            "touch target/ci-core/failure-evidence/extra.bin\n\
             ln -s ../core.log target/ci-core/failure-evidence/core.log\n\
             {FAILURE_EVIDENCE_FIXTURE}"
        );
        check_core_failure_evidence_contract("fixture.yml", &hostile_fixture)?;
        let Some((_, upload)) = hostile_fixture.split_once("    path: |\n") else {
            return Err("fixture missing explicit upload path block".to_string());
        };
        let selected: Vec<&str> = upload
            .lines()
            .take_while(|line| line.starts_with("      "))
            .map(str::trim)
            .collect();
        let expected = vec![
            "${{ steps.core-verdict.outputs.summary_path }}",
            "${{ steps.core-verdict.outputs.metadata_path }}",
        ];
        if selected != expected {
            return Err(format!("unexpected upload selection: {selected:?}"));
        }
        for excluded in ["extra.bin", "core.log", "failure-evidence/"] {
            if selected.iter().any(|path| path.contains(excluded)) {
                return Err(format!(
                    "upload selection retained hostile path: {excluded}"
                ));
            }
        }
        Ok(())
    }
}
