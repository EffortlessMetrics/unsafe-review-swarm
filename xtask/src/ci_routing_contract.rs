//! CI routing-contract gate (`check_ci_routing_contract`).
//!
//! Extracted from `main.rs` as part of #1806 (xtask modularization). Validates
//! that `.github/workflows/ci.yml` keeps the single tight-gate CI contract:
//! required markers for the one self-hosted-primary / gh-hosted-overflow gate,
//! and forbidden markers that would reintroduce the retired size-routed
//! multi-lane pile-of-checks.

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
    // full suite) is the only hard blocker and the only required status check, with
    // ub-review riding along as an advisory LLM layer that consumes the core results
    // as grounding context. The router never blocks the merge and never size-routes.
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
        // Advisory ub-review layer in the same job, fed the fast precontext, with a
        // concise advisory-failure status note.
        "UB Review (advisory)",
        "UB Review advisory status",
        "EffortlessMetrics/ub-review@",
        "mode: intelligent-ci",
        "posting: review",
        "fail-on-gate: false",
        "setup-rust: false",
        "provider-policy: primary-with-fallback",
        "minimax-model: MiniMax-M3",
        "opencode-model: deepseek-v4-flash",
        "pr-thread-context: target/ci-core/precontext.md",
        // Advisory layer must stay non-blocking and fork-safe.
        "continue-on-error: true",
        "github.event.pull_request.head.repo.fork == false",
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
    ] {
        if text.contains(forbidden) {
            return Err(format!(
                "{path} must not reintroduce retired size-routed multi-lane marker: {forbidden}"
            ));
        }
    }
    Ok(())
}
