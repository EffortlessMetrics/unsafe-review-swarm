# Dependency PR Triage — 2026-06-13

Risk-classified triage of open dependency PRs across the swarm workbench (`EffortlessMetrics/unsafe-review-swarm`) and the source-of-record repo (`EffortlessMetrics/unsafe-review`). Verified live state: swarm `main` at `ba3440ed`, published `0.3.6`, latest source release `v0.3.6` (2026-06-11). This is advisory triage — **recommendations only, no merges**. Routing follows `docs/contributing/dependency-pr-policy.md` (risk-routed, not blanket-merged or blanket-deferred; a major bump needs a targeted test for the changed surface; a correct-but-out-of-lane bump is parked, not closed).

## Summary table

| Repo | PR | Dependency | Risk class | CI | Disposition |
|---|---|---|---|---|---|
| swarm | #1620 | (not a dep PR — in-tool RSS via FFI) | governed-unsafe / owner-settled | failing check present | **keep-parked** (do not unpark) |
| swarm | #1565 | github-actions group (ub-review SHA, codecov v6→v7, droid-action SHA) | CI actions (supply-chain + permissions) | 1 check passes but **CONFLICTING** | **blocked** — rebase + re-verify ledger, then merge-now |
| swarm | #1390 | signal-hook 0.3.18→0.4.4 (major) | signal / process control | CLEAN, 4 pass | **verify-existing-suite** (SIGTERM e2e already gates this surface) |
| source | #532 | github-actions group (codecov v6→v7, droid-action SHA) | CI actions | **policy-contracts fails** | **blocked** — missing `workflow-allowlist.toml` ledger entry |
| source | #516 | ra_ap_syntax 0.0.334→0.0.335 | parser / syntax (highest risk) | failing (UNSTABLE) | **blocked / superseded-check** — needs parser/analyzer + calibration evidence; swarm already at 0.0.336 |
| source | #515 | signal-hook 0.3.18→0.4.4 (major) | signal / process control | CLEAN, 4 pass | **verify-existing-suite** — same coverage as #1390 |

**General posture: do not merge any of the risky deps (parser, signal, CI-actions) casually on a green core gate alone.** The core gate does not exercise the changed parser fidelity or workflow permissions — and per the policy doc, the bump is exactly where that behavior moves. For signal-hook, the surface-specific regression suite already exists (see #1390); the remaining task is to confirm it stays green under 0.4, not to author it.

---

## SWARM

### #1620 — in-tool RSS via contracted FFI — KEEP PARKED

`repo: report current and peak RSS in run telemetry via contracted FFI` (author `EffortlessSteven`). Not a dependency PR; it would add `peak_rss_bytes`/`current_rss_bytes` to `RepoScanStatus` via per-platform `extern` FFI in `crates/unsafe-review-core/src/util/peak_rss.rs` (that file does not exist on the current tree — the PR is unmerged), and flips the workspace lint from `unsafe_code = "forbid"` to `"deny"` to permit item-level `#[allow(unsafe_code, reason = …)]`.

This is owner-settled external-first. `docs/contributing/dependency-pr-policy.md` (lines 62–68) explicitly cites `ADR-0008` and this exact pattern: resource measurement is external-first (peak RAM on the scheduled bench harness, no `unsafe` in the shipped binary), "with in-product RSS via a ledgered FFI implemented but parked pending validated demand — an illustration of not spending a governed exception where a free path (external measurement) suffices." CI shows a failing check (`Unsafe Review Rust Result`). **Leave parked. Do not unpark, do not merge, do not implement the lint-posture change.** No risk-class check applies because the decision is governance, not correctness.

### #1565 — github-actions group (3 updates) — BLOCKED (rebase), then merge-now

`chore(deps): bump the github-actions group across 1 directory with 3 updates`. Touches four files (confirmed):
- `.github/workflows/ci.yml`: `EffortlessMetrics/ub-review` SHA bump
- `.github/workflows/coverage.yml`: `codecov/codecov-action@v6` → `@v7`
- `.github/workflows/droid-pr-review.yml`: `Factory-AI/droid-action` SHA bump
- `policy/workflow-allowlist.toml`: ledger entries updated to the new SHAs/version **(ledger correctly re-applied in this PR)**

Risk-class checks (CI actions = pinning + permissions + workflow-behavior):
- **Pinning:** ub-review and droid-action stay pinned to immutable commit SHAs (good); codecov moves to a floating major tag `@v7` (consistent with the existing `@v6` convention in this repo, acceptable but note it is a tag not a SHA).
- **Permissions/behavior:** the `ci.yml` ub-review bump is the structured-ingestion advisory lane; its allowlist `reason` is preserved verbatim except the SHA, so its scope and advisory `continue-on-error` posture are unchanged. codecov v7's substantive change per its release notes is a GPG-key account migration and removal of an internal license-compliance workflow — no permission expansion on our side. droid-action stays advisory/`continue-on-error`.
- **Ledger:** `policy/workflow-allowlist.toml` updated to match — `check-policy` will pass on a clean tree.

**Blocker:** `mergeStateStatus: DIRTY / CONFLICTING`. The one passing check is stale against the conflict. **Recommendation:** rebase (e.g. `@dependabot rebase`), re-confirm the `policy/workflow-allowlist.toml` SHAs/tag still match all three `uses:` lines after rebase (this is the entry most likely to drift in a regroup), then merge once the core gate is green. Low correctness risk; the only real work is the conflict + ledger re-verify.

### #1390 — signal-hook 0.3.18 → 0.4.4 (major) — VERIFY-EXISTING-SUITE

`chore(deps): bump signal-hook from 0.3.18 to 0.4.4`. Diff is minimal: `Cargo.lock` (checksum + version) and `crates/unsafe-review-cli/Cargo.toml` (`signal-hook = "0.3"` → `"0.4"`). CI is CLEAN, 4 checks pass.

Risk-class check (signal/process control = cancellation/timeout/SIGINT tests; `dependency-pr-policy.md` table line 13 and the major-bump rule lines 18–23): the consumer is the repo-scan interrupt path in `crates/unsafe-review-cli/src/execute.rs`:
- `signal_hook::consts::signal::{SIGINT, SIGTERM}` (line 9)
- `signal_hook::iterator::{Handle as SignalHandle, Signals}` (line 11)
- `Signals::new([SIGTERM, SIGINT])`, `.handle()`, `signals.forever().next()` (lines 838–842), `handle.close()` in the `RepoSignalGuard` `Drop` (line 796)

The 0.4.0 breaking change is `low_level::pipe` moving to `OwnedFd` — **not on the path this code uses** (the `iterator::Signals` API used here is stable across 0.3→0.4), so the bump is plausibly behavior-preserving. Crucially, this surface is **already gated by targeted integration tests** that drive the built binary and send a real signal — they live in the facade crate `crates/unsafe-review/tests/e2e.rs`, not in `crates/unsafe-review-cli/tests/`:
- `repo_sigterm_writes_interrupted_status_sidecar` (line 5212) — sends `kill -TERM`, asserts exit code 143, `phase=terminated`, `status["signal"]=="SIGTERM"`, `stop_reason=="terminated"`, and that no partial report is invented before rendering.
- `repo_sigterm_keeps_completed_file_partial_report` (line 5285) — same after a completed file, asserting the `<out>.partial` report is retained.
- `repo_timeout_keeps_completed_file_partial_snapshot` (line 4840) — the timeout twin of the partial-status contract.

These exercise the `phase=terminated` partial-status contract (`execute.rs` lines ~838–890, surfaced in help text line 3323 writing `<out>.status.json` and `<out>.partial`) through a real OS signal via the `UNSAFE_REVIEW_INTERNAL_REPO_SIGNAL_TEST_PAUSE_MS` hook. So the policy's targeted-test requirement for this surface is **already met for SIGTERM**.

**Remaining (minor) gap:** the tests only deliver `kill -TERM`; SIGINT is registered on the same `Signals::new([SIGTERM, SIGINT])` / single `forever().next()` path but is not exercised by a `kill -INT` test, so SIGINT delivery is structurally covered but not directly asserted. **Recommendation:** run the existing facade-crate signal e2e suite against the 0.4 bump (`cargo test -p unsafe-review --test e2e repo_sigterm --locked`) and confirm green; optionally add a `kill -INT` variant asserting `status["signal"]=="SIGINT"` to close the SIGINT-delivery gap. If the suite is green under 0.4, this is a merge candidate (after the source/swarm sequencing in the cross-cutting notes) — it does not need to be parked pending a test that already exists.

---

## SOURCE (`EffortlessMetrics/unsafe-review`)

Treat with extra caution — source is the release/source-of-record repo; only curated promotions land here.

### #532 — github-actions group (2 updates) — BLOCKED (missing ledger entry)

`chore(deps): bump the github-actions group with 2 updates` (codecov `@v6`→`@v7`, Factory-AI/droid-action SHA bump). CI: **`policy-contracts` fails** and `Unsafe Review Rust Result` fails; `mergeStateStatus: UNSTABLE`.

Risk-class check (CI actions = pinning + permissions + behavior + `policy/workflow-allowlist.toml`): the diff touches **only** `.github/workflows/coverage.yml` and `.github/workflows/droid-pr-review.yml` (confirmed) — it does **not** update `policy/workflow-allowlist.toml`. The failing `policy-contracts` log is explicit:

> `xtask: .github/workflows/coverage.yml uses action codecov/codecov-action@v7 that is not listed in policy/workflow-allowlist.toml` → exit code 2

Confirmed: the source repo's `policy/workflow-allowlist.toml` still lists `codecov/codecov-action@v6`. **This is a hard, self-inflicted block, not a runtime regression.** Contrast with swarm #1565, which *does* carry the ledger update — Dependabot did not regenerate the ledger here.

**Recommendation:** blocked until the ledger entry is updated. Either (a) push a follow-up commit to this PR bumping `codecov/codecov-action@v6`→`@v7` and the droid-action SHA in `policy/workflow-allowlist.toml`, or (b) regenerate via the swarm→source promotion so the ledger lands with the workflow change. Do not merge with the policy gate red. Same pinning/permission notes as #1565 apply (codecov tag-pinned, droid SHA-pinned, both advisory).

### #516 — ra_ap_syntax 0.0.334 → 0.0.335 — BLOCKED / SUPERSEDED-CHECK (highest risk)

`build(deps): bump ra_ap_syntax from 0.0.334 to 0.0.335`. **This is the syntax backend and the single highest-risk dependency in the queue.** CI: failing, `mergeStateStatus: UNSTABLE` (`Unsafe Review Rust Result` and the routed `Rust Small` jobs fail; the `Result` job largely echoes the propagated route-failure). Diff touches only `Cargo.lock` and `crates/unsafe-review-core/Cargo.toml` (confirmed).

Risk-class check (parser/syntax = analyzer/parser tests green; `dependency-pr-policy.md` table line 12): `ra_ap_syntax` backs the stable-first source parser per `ADR-0001` and is consumed directly in `crates/unsafe-review-core/src/analysis/syntax.rs` (`use ra_ap_syntax::{AstNode, Edition, SourceFile}` line 1, `TextSize` conversion at line 97). Per the policy, *"a parser change can silently shift detection"* — a parser bump can change which spans/nodes the analyzer sees and therefore which unsafe seams produce cards, with no compile error to flag it. The required evidence is **the full analyzer/parser test suite green plus the calibration gate** (`check-calibration` cross-checks the operation registry against the analyzer), and ideally a fixture-corpus diff to confirm no card-detection drift.

**Staleness / supersession flag (important):** the swarm checkout already pins `ra_ap_syntax = "0.0.336"` in `crates/unsafe-review-core/Cargo.toml`, while source PR #516 only moves source from `0.0.334` → `0.0.335` (both pins confirmed). Source is two patch releases behind swarm on the parser. Do not merge #516 blindly: confirm whether the intended source target should be `0.0.335` or whether it should jump straight to `0.0.336` to match the proven swarm pin (the prior bump `0.0.333`→`0.0.334` is recorded as merged in `docs/handoffs/2026-05-26-source-dependabot-sync.md`). **Recommendation:** blocked. Rebase/recreate, decide the target version against the swarm `0.0.336` pin, then require the green analyzer/parser + calibration suite and a no-drift fixture-corpus check before merge. Highest scrutiny of any PR here.

### #515 — signal-hook 0.3.18 → 0.4.4 (major) — VERIFY-EXISTING-SUITE

`chore(deps): bump signal-hook from 0.3.18 to 0.4.4`. The source-repo twin of swarm #1390; identical minimal diff (`Cargo.lock` + `crates/unsafe-review-cli/Cargo.toml` `"0.3"`→`"0.4"`). CI is CLEAN, 4 checks pass, `mergeStateStatus: CLEAN`. Source currently pins `signal-hook = "0.3"` (confirmed).

Risk-class check is the same as #1390: major bump, signal/process-control surface, and the same `iterator::Signals`/`Handle` API path applies (the 0.4.0 `low_level::pipe` `OwnedFd` break is off-path). The same SIGTERM regression coverage applies once promoted — the facade-crate `repo_sigterm_*` tests are the gate. **Recommendation:** verify-existing-suite; do not merge until the facade-crate signal e2e suite is confirmed green under 0.4 in source. Because #1390 (swarm) and #515 (source) are the same bump and the gating tests already exist on swarm, verify on swarm #1390 first, then promote the bump (and any new SIGINT-delivery assertion) to source alongside #515 — do not land the source bump first.

---

## Cross-cutting recommendations

1. **Do not casual-merge any risky dep on a green core gate.** ra_ap_syntax (#516) needs the parser/analyzer + calibration + no-drift evidence the policy doc names; the core gate does not cover it. signal-hook (#1390/#515) is the one risky surface whose targeted test already exists — gate it on that existing suite, not on a fresh authoring task.
2. **CI-actions PRs must carry their `policy/workflow-allowlist.toml` ledger update in the same PR.** #1565 does (just needs a rebase + re-verify); #532 does not and is hard-blocked by `check-policy` because of it — this is the fix, not a flaky runner.
3. **Sequence the two signal-hook PRs:** confirm the existing facade-crate SIGTERM e2e suite is green under 0.4 on swarm #1390 first (and optionally add a direct SIGINT-delivery assertion), then promote bump + any new test to source #515. Do not let the source bump lead.
4. **ra_ap_syntax #516 is the highest-risk item and is also stale vs. the swarm's `0.0.336` pin** — resolve the target version and require analyzer/parser + calibration + fixture-no-drift evidence before any merge.
5. **#1620 stays parked** — owner-settled external-first, explicitly documented in `dependency-pr-policy.md`/`ADR-0008`. Do not unpark.

No merges performed. All merge actions are owner decisions. Update `docs/contributing/dependency-pr-policy.md` "Currently parked" list (lines 32–37) to reflect this triage when these dispositions are actioned — and remove #1390 from the parked list once its existing SIGTERM suite is confirmed green under 0.4, since its blocking rationale (no targeted test) no longer holds.
