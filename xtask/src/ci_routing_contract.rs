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
