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

/// Require private evidence staging to recreate its owner-only parent directly
/// after cleanup and before allocating the unpredictable staging directory.
fn check_private_staging_order(path: &str, text: &str) -> Result<(), String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let cleanup = "rm -rf \"$failure_root\"";
    let create = "mkdir -m 700 \"$failure_root\"";
    let allocate = "failure_dir=\"$(mktemp -d \"${failure_root}/staging-XXXXXX\")\"";
    let cleanup_index = lines
        .iter()
        .position(|line| *line == cleanup)
        .ok_or_else(|| format!("{path} missing private staging cleanup: {cleanup}"))?;
    if lines.get(cleanup_index + 1) != Some(&create) {
        return Err(format!(
            "{path} must recreate private staging root immediately after cleanup: {create}"
        ));
    }
    if lines.get(cleanup_index + 2) != Some(&allocate) {
        return Err(format!(
            "{path} must allocate private staging immediately after owner-only root creation: {allocate}"
        ));
    }
    Ok(())
}

/// Require the structured test invocation to be part of the launched core
/// gate, rather than accepting the `ci-test` prefix from the validator command.
fn check_core_gate_launch_order(path: &str, text: &str) -> Result<(), String> {
    let launch = r#"_step test env UNSAFE_REVIEW_CI_HANDOFF_DIR="${RUNNER_TEMP}/unsafe-review-structured-${CORE_RUN_KEY}" cargo run --locked -p xtask -- ci-test"#;
    let launch_index = text
        .find(launch)
        .ok_or_else(|| format!("{path} missing exact launched ci-test invocation: {launch}"))?;
    let background = text
        .find(") > target/ci-core/core.log 2>&1 &")
        .ok_or_else(|| format!("{path} missing background core-gate launch"))?;
    let verdict = text
        .find("- name: Assert core gate verdict")
        .ok_or_else(|| format!("{path} missing core-gate verdict step"))?;
    if !(launch_index < background && background < verdict) {
        return Err(format!(
            "{path} must launch the exact ci-test invocation before asserting the core verdict"
        ));
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
            // Diff scoping consumes the runner-provided environment value as a
            // quoted argument and fails closed to the full test path.
            "base_ref=\"${GITHUB_BASE_REF:-main}\"",
            "git diff --name-only \"origin/${base_ref}...HEAD\"",
            "_changed_rs=\"__diff_unavailable__\"",
            // The shipped Bash arithmetic must produce the numeric elapsed TSV
            // field consumed by the closed-vocabulary evidence filter.
            "_now=$(date +%s)",
            "_elapsed=$((_now - _s))",
            // Only closed-vocabulary step status reaches the bounded artifact.
            "step_id\\telapsed_seconds\\texit_status",
            "$1 ~ /^(fmt|clippy|test|check-pr)$/",
            "case \"$core_mode\" in",
            "head -n 80",
            "head -c 16384",
            // Staging is recreated after the core finishes in a private,
            // unpredictable runner-temp path. Only the baseline two files plus
            // the optional sanitized structured diagnostic are exposed.
            r#"_step test env UNSAFE_REVIEW_CI_HANDOFF_DIR="${RUNNER_TEMP}/unsafe-review-structured-${CORE_RUN_KEY}" cargo run --locked -p xtask -- ci-test"#,
            "UNSAFE_REVIEW_CI_HANDOFF_DIR=\"${RUNNER_TEMP}/unsafe-review-structured-${CORE_RUN_KEY}\"",
            "id: core-verdict",
            "${RUNNER_TEMP}/unsafe-review-core-evidence-${CORE_RUN_KEY}",
            "rm -rf \"$failure_root\"",
            "mktemp -d \"${failure_root}/staging-XXXXXX\"",
            "excerpt_path=\"${failure_dir}/step-status.txt\"",
            "excerpt_tmp=\"$(mktemp \"${failure_dir}/.step-status-XXXXXX\")\"",
            "if ! {",
            "elif ! mv \"$excerpt_tmp\" \"$excerpt_path\"; then",
            "|| [ -L \"$excerpt_path\" ]; then",
            "[ ! -L \"$failure_summary_path\" ]",
            "[ ! -L \"$failure_metadata_path\" ]",
            "summary_path=$failure_summary_path",
            "metadata_path=$failure_metadata_path",
            "diagnostics_path=$diagnostics_path",
            "diagnostics_source=\"${RUNNER_TEMP}/unsafe-review-structured-${CORE_RUN_KEY}/test-diagnostics.json\"",
            "cp --no-dereference \"$diagnostics_source\" \"$diagnostics_path\"",
            "cargo run --locked -p xtask -- ci-test-validate \"$diagnostics_path\"",
            "if [ \"${core_exit}\" != \"0\" ]; then",
            // Artifact retention is always attempted but cannot change the job.
            "name: Upload bounded core-gate failure evidence",
            "${{ always() &&",
            "continue-on-error: true",
            "uses: actions/upload-artifact@v7",
            "path: |",
            "${{ steps.core-verdict.outputs.summary_path }}",
            "${{ steps.core-verdict.outputs.metadata_path }}",
            "${{ steps.core-verdict.outputs.diagnostics_path }}",
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
            "target/ci-core/redacted-excerpt.txt",
            "origin/${{ github.base_ref",
        ],
    )?;
    check_private_staging_order(path, text)?;
    check_core_gate_launch_order(path, text)
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
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    const FAILURE_EVIDENCE_FIXTURE: &str = r#"
CORE_RUN_KEY: ${{ github.run_id }}-${{ github.run_attempt }}
core_exit_path="target/ci-core/core_exit-${CORE_RUN_KEY}"
cargo run --locked -p xtask -- ci-test
UNSAFE_REVIEW_CI_HANDOFF_DIR="${RUNNER_TEMP}/unsafe-review-structured-${CORE_RUN_KEY}"
base_ref="${GITHUB_BASE_REF:-main}"
git diff --name-only "origin/${base_ref}...HEAD"
_changed_rs="__diff_unavailable__"
_now=$(date +%s)
_elapsed=$((_now - _s))
rm -f target/ci-core/core_exit "$core_exit_path"
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
_step test env UNSAFE_REVIEW_CI_HANDOFF_DIR="${RUNNER_TEMP}/unsafe-review-structured-${CORE_RUN_KEY}" cargo run --locked -p xtask -- ci-test \
  ) > target/ci-core/core.log 2>&1 &
- name: Assert core gate verdict
  id: core-verdict
if [ "${core_exit}" != "0" ]; then
  failure_root="${RUNNER_TEMP}/unsafe-review-core-evidence-${CORE_RUN_KEY}"
  rm -rf "$failure_root"
  mkdir -m 700 "$failure_root"
  failure_dir="$(mktemp -d "${failure_root}/staging-XXXXXX")"
  excerpt_path="${failure_dir}/step-status.txt"
  excerpt_tmp="$(mktemp "${failure_dir}/.step-status-XXXXXX")"
failure_summary_path="${failure_dir}/summary.md"
failure_metadata_path="${failure_dir}/metadata.json"
diagnostics_source="${RUNNER_TEMP}/unsafe-review-structured-${CORE_RUN_KEY}/test-diagnostics.json"
diagnostics_path="${failure_dir}/test-diagnostics.json"
cp --no-dereference "$diagnostics_source" "$diagnostics_path"
cargo run --locked -p xtask -- ci-test-validate "$diagnostics_path"
  if ! {
    printf 'step_id\telapsed_seconds\texit_status\n'
  } > "$excerpt_tmp"; then
    staging_ready="false"
  elif ! mv "$excerpt_tmp" "$excerpt_path"; then
    staging_ready="false"
  elif [ ! -f "$excerpt_path" ] || [ -L "$excerpt_path" ]; then
    staging_ready="false"
  fi
  if [ -f "$failure_summary_path" ] && [ ! -L "$failure_summary_path" ] \
    && [ -f "$failure_metadata_path" ] && [ ! -L "$failure_metadata_path" ]; then
    echo "summary_path=$failure_summary_path" >> "$GITHUB_OUTPUT"
    echo "metadata_path=$failure_metadata_path" >> "$GITHUB_OUTPUT"
    echo "diagnostics_path=$diagnostics_path" >> "$GITHUB_OUTPUT"
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
      ${{ steps.core-verdict.outputs.diagnostics_path }}
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
    fn diagnostics_are_validated_before_output_exposure() -> Result<(), String> {
        let text = FAILURE_EVIDENCE_FIXTURE;
        let copy = text
            .find("cp --no-dereference \"$diagnostics_source\" \"$diagnostics_path\"")
            .ok_or_else(|| "diagnostics copy step is missing".to_string())?;
        let validate = text
            .find("cargo run --locked -p xtask -- ci-test-validate \"$diagnostics_path\"")
            .ok_or_else(|| "diagnostics validator step is missing".to_string())?;
        let expose = text
            .find("echo \"diagnostics_path=$diagnostics_path\" >> \"$GITHUB_OUTPUT\"")
            .ok_or_else(|| "diagnostics output exposure is missing".to_string())?;
        if !(copy < validate && validate < expose) {
            return Err("diagnostics were exposed before validation completed".to_string());
        }
        if !text.contains("${{ steps.core-verdict.outputs.diagnostics_path }}") {
            return Err("diagnostics_path is missing from the upload vector".to_string());
        }
        Ok(())
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
    fn shipped_elapsed_expression_emits_numeric_retained_tsv_row() -> Result<(), String> {
        let workflow_path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows/ci.yml");
        let workflow = std::fs::read_to_string(&workflow_path)
            .map_err(|error| format!("failed to read {}: {error}", workflow_path.display()))?;
        let now_expression = workflow
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("_now="))
            .ok_or_else(|| "live workflow missing _now expression".to_string())?;
        let elapsed_expression = workflow
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with("_elapsed="))
            .ok_or_else(|| "live workflow missing _elapsed expression".to_string())?;
        if now_expression != "_now=$(date +%s)" {
            return Err(format!(
                "unexpected live current-time expression: {now_expression:?}"
            ));
        }
        if elapsed_expression != "_elapsed=$((_now - _s))" {
            return Err(format!(
                "unexpected live elapsed expression: {elapsed_expression:?}"
            ));
        }

        let script = format!(
            "_s=$(date +%s)\n{now_expression}\n{elapsed_expression}\nprintf 'check-pr\\t%s\\t0\\n' \"$_elapsed\"\n"
        );
        let mut child = Command::new("bash")
            .arg("-s")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("failed to start live elapsed expression: {error}"))?;
        child
            .stdin
            .take()
            .ok_or_else(|| "bash stdin was not available".to_string())?
            .write_all(script.as_bytes())
            .map_err(|error| format!("failed to send live elapsed expression: {error}"))?;
        let output = child
            .wait_with_output()
            .map_err(|error| format!("failed to execute live elapsed expression: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "live elapsed expression failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        let row = String::from_utf8(output.stdout)
            .map_err(|error| format!("elapsed TSV row was not UTF-8: {error}"))?;
        let elapsed = row
            .trim_end()
            .split('\t')
            .nth(1)
            .ok_or_else(|| format!("elapsed TSV row missing elapsed field: {row:?}"))?;
        if elapsed.is_empty() || !elapsed.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!("elapsed TSV field is not numeric: {elapsed:?}"));
        }
        let summary = bounded_core_status_summary(&row);
        if !summary.contains(&format!("check-pr\t{elapsed}\t0")) {
            return Err(format!(
                "numeric elapsed TSV row was not retained: {summary:?}"
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
            "${{ steps.core-verdict.outputs.diagnostics_path }}",
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

    #[test]
    fn rejects_predictable_workspace_excerpt_symlink_path() -> Result<(), String> {
        let hostile_fixture = format!(
            "printf 'raw-secret-core-log' > target/ci-core/core.log\n\
             ln -s core.log target/ci-core/redacted-excerpt.txt\n\
             excerpt_path=\"target/ci-core/redacted-excerpt.txt\"\n\
             {FAILURE_EVIDENCE_FIXTURE}"
        );
        let Err(error) = check_core_failure_evidence_contract("fixture.yml", &hostile_fixture)
        else {
            return Err("predictable workspace excerpt fixture unexpectedly passed".to_string());
        };
        if !error.contains("target/ci-core/redacted-excerpt.txt") {
            return Err(format!("unexpected workspace-excerpt error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_private_staging_without_owner_only_root_creation() -> Result<(), String> {
        let missing_create =
            FAILURE_EVIDENCE_FIXTURE.replace("  mkdir -m 700 \"$failure_root\"\n", "");
        let Err(error) = check_core_failure_evidence_contract("fixture.yml", &missing_create)
        else {
            return Err("missing private staging root creation unexpectedly passed".to_string());
        };
        if !error.contains("mkdir -m 700 \"$failure_root\"") {
            return Err(format!("unexpected private-root error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_private_staging_root_creation_after_mktemp() -> Result<(), String> {
        let reordered = FAILURE_EVIDENCE_FIXTURE.replace(
            "  mkdir -m 700 \"$failure_root\"\n  failure_dir=\"$(mktemp -d \"${failure_root}/staging-XXXXXX\")\"",
            "  failure_dir=\"$(mktemp -d \"${failure_root}/staging-XXXXXX\")\"\n  mkdir -m 700 \"$failure_root\"",
        );
        let Err(error) = check_core_failure_evidence_contract("fixture.yml", &reordered) else {
            return Err("reordered private staging root creation unexpectedly passed".to_string());
        };
        if !error.contains("immediately after cleanup") {
            return Err(format!("unexpected staging-order error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_raw_github_base_ref_template_in_shell() -> Result<(), String> {
        let injected = format!(
            "_changed_rs=$(git diff --name-only origin/${{{{ github.base_ref || 'main' }}}}...HEAD)\n{FAILURE_EVIDENCE_FIXTURE}"
        );
        let Err(error) = check_core_failure_evidence_contract("fixture.yml", &injected) else {
            return Err("raw github.base_ref template unexpectedly passed".to_string());
        };
        if !error.contains("origin/${{ github.base_ref") {
            return Err(format!("unexpected base-ref template error: {error}"));
        }
        Ok(())
    }

    #[test]
    fn rejects_diff_scope_that_does_not_force_full_tests_on_failure() -> Result<(), String> {
        let fail_open = FAILURE_EVIDENCE_FIXTURE.replace(
            "_changed_rs=\"__diff_unavailable__\"\n",
            "_changed_rs=\"\"\n",
        );
        let Err(error) = check_core_failure_evidence_contract("fixture.yml", &fail_open) else {
            return Err("fail-open diff scope fixture unexpectedly passed".to_string());
        };
        if !error.contains("_changed_rs=\"__diff_unavailable__\"") {
            return Err(format!("unexpected diff fail-closed error: {error}"));
        }
        Ok(())
    }
}
