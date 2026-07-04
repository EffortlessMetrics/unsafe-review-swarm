//! CI routing-contract gate (`check_ci_routing_contract`).
//!
//! Extracted from `main.rs` as part of #1806 (xtask modularization). Validates
//! that `.github/workflows/ci.yml` keeps the single tight-gate CI contract:
//! required markers for the one self-hosted-primary / gh-hosted-overflow gate,
//! and forbidden markers that would reintroduce the retired size-routed
//! multi-lane pile-of-checks. Also validates that the standalone advisory
//! ub-review lane (`.github/workflows/ub-review.yml`) keeps its non-blocking
//! advisory posture: SHA-pinned action, continue-on-error, fail-on-gate off,
//! and fork-guarded secret use.

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
    for needle in [
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
        // Fast precontext launches the LLM lanes off cheap signal, the deterministic
        // core gate runs concurrently in the background (guarded by a disk-headroom
        // check), and the final assert decides the merge on the core verdict.
        "Fast precontext and launch core gate",
        "cargo run --locked -p xtask -- check-pr",
        "df -h",
        "core_exit",
        "Assert core gate verdict",
        // The router must stay fork-safe: fork PRs always overflow to gh-hosted.
        "github.event.pull_request.head.repo.fork",
    ] {
        if !text.contains(needle) {
            return Err(format!(
                "{path} missing required single-gate CI contract marker: {needle}"
            ));
        }
    }
    // The capacity router is back, but ONLY in its minimal self-hosted-primary /
    // gh-overflow shape. The OLD size-routed multi-lane pile-of-checks must not
    // reappear: no per-size lanes (cpx42/cx43/cx53), no separate normalized "Rust
    // Small Result" required check, no budget opt-out fallback modes, and no
    // repository-level runner discovery or the broken Docker Rust Small image.
    if text.contains("repos/${") && text.contains("/actions/runners") {
        return Err(format!(
            "{path} must not reintroduce repository runner discovery (org-level only)"
        ));
    }
    for forbidden in [
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
    ] {
        if text.contains(forbidden) {
            return Err(format!(
                "{path} must not reintroduce retired CI shape marker: {forbidden}"
            ));
        }
    }
    check_ub_review_advisory_contract()
}

/// Validate the standalone advisory ub-review lane contract in
/// `.github/workflows/ub-review.yml`: the action stays SHA-pinned, the job
/// stays non-blocking (continue-on-error, fail-on-gate off, bounded timeout),
/// and org-secret use stays guarded to same-repo PRs. The lane must never
/// become a required check; branch protection names only
/// "Unsafe Review Rust Result" from ci.yml.
fn check_ub_review_advisory_contract() -> Result<(), String> {
    let path = ".github/workflows/ub-review.yml";
    let text =
        std::fs::read_to_string(path).map_err(|err| format!("failed to read {path}: {err}"))?;
    for needle in [
        // SHA-pinned advisory action; a re-tag cannot silently change the lane.
        "uses: EffortlessMetrics/ub-review@",
        // Non-blocking advisory posture.
        "continue-on-error: true",
        "fail-on-gate: 'false'",
        "timeout-minutes:",
        // Same-repo guard: fork PRs cannot read the MINIMAX_API_KEY org secret.
        "github.event.pull_request.head.repo.full_name == github.repository",
        // Posts one grouped advisory review; the only reason for
        // pull-requests: write in this workflow.
        "posting: review",
        "pull-requests: write",
    ] {
        if !text.contains(needle) {
            return Err(format!(
                "{path} missing required advisory ub-review lane marker: {needle}"
            ));
        }
    }
    for forbidden in [
        // The advisory lane must never gain gate authority or extra tokens.
        "fail-on-gate: 'true'",
        "fail-on-gate: true",
        "checks: write",
        "contents: write",
        "issues: write",
    ] {
        if text.contains(forbidden) {
            return Err(format!(
                "{path} must not carry blocking or write-scope marker: {forbidden}"
            ));
        }
    }
    Ok(())
}
