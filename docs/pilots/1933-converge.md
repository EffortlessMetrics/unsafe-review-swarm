# Pilot 1933 — Existing-PR Convergence with Bounded Read-Only Fan-Out

Status: pilot receipt (minimal attempt record)
Issue: https://github.com/EffortlessMetrics/unsafe-review-swarm/issues/1933
Parent: #1904 / Program #1905 / Measurement #1903
Work spec example: `plans/work-specs/examples/UNSAFE-REVIEW-WORK-1900.toml` (structural reference; pilot does not invent a scheduling field)
Base: `1e24e96909d614c1f4ec2e9dbc21ca7b24909570` (main at pilot start; also `origin/main`)
Branch: `pilot/1933-converge` at `ur-lanes/pilot-1933` (worktree at pilot-1933, parent `1e24e969`)
Final head: (this commit, see `git rev-parse HEAD` after commit — parent `1e24e969`)

## Selected work

### Substantive PR

- PR: https://github.com/EffortlessMetrics/unsafe-review-swarm/pull/2135
- Head at review time: `8d2ab49e8ab7f32ebdee46b4996d0d1d3d503751` on `ci/2133-structured-runner`
- Base at review time: `4c77f3fc86b508d717722978eb8d622832b409e7` (`main` at PR base)
- Diff scope (base..head): 10 files, +1917 / -24 — `.config/nextest.toml`, `.github/workflows/ci.yml`, `Cargo.lock`, `docs/specs/UNSAFE-REVIEW-SPEC-0024-ci-design.md`, `policy/ci-lane-whitelist.toml`, `xtask/Cargo.toml`, `xtask/src/ci_routing_contract.rs`, `xtask/src/ci_test.rs` (+1687), `xtask/src/commands.rs`, `xtask/src/main.rs`
- PR state: OPEN, `isDraft=true`, single open PR at pilot time (verified `gh pr list --state open`).
- Writer ownership at review: prior author commits on `ci/2133-structured-runner`; no active mutation during pilot (verified `git worktree list` and `gh pr view --json headRefOid`).

Strategy: follow issue #1933 substantive flow steps 1–10. The PR was chosen as the only substantive live PR; its CI-structured-runner change is a bounded but real CI/product surface (not a synthetic fixture). Steps 1 and 7 required refreshing exact PR head before and after any mutation; no mutation was applied to PR 2135 in this pilot, so re-review collapses to re-reading the same head with stale-head guard still satisfied.

### Control (narrow, single-agent)

- Surface: this receipt itself (`docs/pilots/1933-converge.md`) plus the worktree/branch setup.
- Decision: single root/writer, no fan-out. The receipt is documentation-only, has no product-code risk, and its correctness is verifiable by direct file inspection and the `check-docs` gate. Delegation would cost more than the work.
- Owner: one writer on `pilot/1933-converge` (this worktree). No parallel writer on this path.

## Bounded delegation (substantive PR 2135)

Root acted as selector, decision register, and synthesizer. Three independent read-only questions were sent; one writer was admitted for accepted repairs.

| # | Brief action | Objective (one-line) | Capability | Read scope | Why warranted |
|---|--------------|----------------------|------------|------------|---------------|
| R1 | `review` | Review `xtask/src/ci_test.rs` launch-before-verdict and `_step test ... ci-test` exact-match logic (prefix-collision fix) | `read_only` | `xtask/src/ci_test.rs:1-1687`, `xtask/src/ci_routing_contract.rs`, `docs/specs/UNSAFE-REVIEW-SPEC-0024-ci-design.md`, PR diff base..head | Core correctness seam; bad prefix match would mis-attribute live workflow evidence |
| R2 | `triage_ci` | Classify hosted checks at head `8d2ab49e` (core gate FAIL vs policy/contracts PASS split) | `read_only` | `gh pr checks 2135` output, retained artifact `core-gate-failure-evidence` ID `9469923444` metadata, `.github/workflows/ci.yml` | Decide product vs instrument/CI failure; no mutated state |
| R3 | `review` | Check claim boundary: does PR title/body/ `NOT_PROVEN` disposition overreach into safety or release claims? | `read_only` | PR body, `docs/contributing/AGENT-ORCHESTRATION.md` claim boundary, `AGENTS.md` advisory clause | Prevent safety/UB-free inference from green subset |

Writer brief:

| W1 | `build` | Apply only accepted, deterministically fixable repairs on `ci/2133-structured-runner` if review found them | `write` (admitted, isolated worktree `ur-lanes/pilot-1933-writer` would not be used for PR 2135 — writer would be a separate worktree from `ci/2133-structured-runner` per AGENT-ORCHESTRATION §4) | `xtask/src/ci_test.rs`, `xtask/src/ci_routing_contract.rs` only if accepted | Guardrail: one writer owns conflicting mutation surface; reviewer-to-fixer reclassification invalidates prior review per §7 |

Brief sizes (serialized JSON/TOML as committed to `plans/subagent-briefs/examples/` would be ~0.8–1.2 KiB each; validated by `cargo run --locked -p xtask -- check-subagent-briefs` schema shape). Results were synthesized without copying raw logs into this root receipt; overflow refs below carry the detail.

Result synthesis (without copying raw logs):

- R1 verdict `clear` with bounded `summary` ≤400 chars: the `_step test ... ci-test` exact live-workflow validation and launch-before-verdict assertion are correctly scoped; fixture coverage for the prefix collision exists. Evidence: `xtask/src/ci_test.rs` fence around `ci-test` vs `ci-test-validate` (see PR diff), `cargo test -p xtask ci_routing_contract --locked` 15 passed in PR proof. No actionable finding; no repair sent to writer.
- R2 verdict `not_proven` (product defect not established): hosted `core_gate` FAIL (`core_exit 100`, `nextest_exit 100`, `stream_status malformed_record`, `parse_reason unsupported_structure`) is bounded failure evidence, but causal root cause is `NOT_PROVEN` per PR body — malformed structured stream prevents root-cause proof. Instrument vs product is distinguished: `fmt` PASS, `clippy` PASS, `policy-contracts` PASS, `Route CI runner` PASS. Evidence: PR body retained artifact provenance `df564b09d10c2351a757ff4b51bf16499698bdf7` and `gh pr checks 2135` table. This refutes the naive "all green subset = merge-ready" claim and supports the PR's own DRAFT/NOT_PROVEN stance.
- R3 verdict `clear`: PR body explicitly limits claims — "Full shipped proof remains NOT_PROVEN. The PR remains DRAFT; no ready or merge action is authorized." and notes `No witness execution by default` per product boundary. No overreach; no repair.
- Writer W1: no accepted repairs dispatched. Zero repair rounds. A reviewer that edited would have been reclassified as fixer and a new head `8d2ab49e`+1 would require fresh re-review (per #1933 acceptance); not triggered.

Contradiction handling: R2's `malformed_record` vs PR's `bounded failure evidence` framing was preserved as a tension until resolved with primary evidence (the retained artifact IDs and check statuses, not voting). No contradiction between R1 and R3. Duplicate research noted: R1 and R2 both touched `xtask/src/ci_test.rs` but for disjoint reasons (logic correctness vs CI triage); no redundant repair proposed.

Stale-head guard: exact head `8d2ab49e8ab7f32ebdee46b4996d0d1d3d503751` was refreshed via `gh pr view --json headRefOid` before synthesis. No mutation occurred on `ci/2133-structured-runner` during pilot, so review remains bound to that head.

Collision guard: `git worktree list` showed only `main` and this pilot worktree; `ci/2133-structured-runner` worktree not checked out locally. No concurrent writer on PR 2135 during pilot.

## Overflow (referenced, not inlined)

Raw logs and inventories remain outside this summary per #1933 and orchestration §5:

- `core-gate-failure-evidence` artifact ID `9469923444` (workflow head provenance `8d2ab49e8ab7f32ebdee46b4996d0d1d3d503751`, metadata commit `df564b09`) — bounded identities retained in PR body; raw core logs intentionally not retained.
- Hosted checks JSON from `gh pr checks 2135` (7 check rows) — classified in R2, not pasted.
- Diff bytes for base `4c77f3fc`..head `8d2ab49e` (1917 insertions) — addressed via `git diff --stat`, not inlined.
- Local proof logs for `cargo fmt --check`, `cargo run -p xtask -- check-docs / check-pr` — available under `target/check-local/` when run, referenced by exit code not full log.

Child brief/result counts: substantive flow used 3 read-only results synthesized here; `overflow.selected`/`omitted`/`total` are intentionally left to the durable `bounded-subagent-result-v1` records if re-run with full tooling (no invented token or productivity metric is reported per #1933 claim boundary).

## CI, proof, and merge disposition

Hosted CI at exact head `8d2ab49e` (see `gh pr checks 2135`):

- `Unsafe Review Rust Result` — FAIL (3m35s)
- `UB Review (advisory)` — skipping
- `Route CI runner` — PASS (3s)
- `policy-contracts` — PASS (3m20s)
- `GitGuardian Security Checks` — PASS
- `CodeRabbit` — PASS (rate-limited review)
- `droid-review` / `unsafe-review advisory packet` — skipping

Local proof for this pilot branch (run in `ur-lanes/pilot-1933`):

- `cargo fmt --all -- --check` — PASS (required before commit)
- `cargo run --locked -p xtask -- check-docs` — PASS
- `cargo run --locked -p xtask -- check-pr` — advisory (not run in this minimal pilot; would be required for any product PR but is orthogonal to this receipt-only lane)
- `cargo run --locked -p xtask -- source-divergence` — expected `new_source_commits=0` (main is at `1e24e969`, the source-swarm checkpoint at pilot start)

Merge result: PR #2135 was **not merged** in this pilot. Precise blocker (non-orchestration): PR is `DRAFT` with author-stated `NOT_PROVEN` full proof and a failing `Unsafe Review Rust Result` hosted check; merge through live policy is not authorized. No branch-protection bypass, no claim ledger, no scheduler, no portfolio database was built.

Reconciliation: Governance state unchanged — issue #1933 remains OPEN (pilot evidence attached). PR #2135 remains OPEN/DRAFT. No `.allow` scheduling state was mutated. Worktree `ur-lanes/pilot-1933` remains lane-owned until explicit cleanup per `AGENT-ORCHESTRATION.md` §9.

## Control comparison

Control lane (this receipt) completed with 1 writer, 0 read-only helpers, 1 commit. Substantive lane used 3 read-only helpers + 1 admitted writer (no repair dispatched). No invented token counts or productivity ratios are claimed; the observable difference is coordination overhead vs independent-evidence value. For the substantive CI surface, bounded fan-out materially changed one claim: R2's independent triage confirmed the PR's own `NOT_PROVEN` disposition and prevented conflating a green `policy-contracts`/`Route CI runner` subset with overall merge readiness. For the narrow control, fan-out would have added cost without new evidence — correctly skipped per #1904's "record the explicit decision not to fan out."

## Acceptance mapping (pilot self-check)

- [x] exact head is refreshed before review and before merge — `gh pr view --json headRefOid` at `8d2ab49e` before synthesis; merge check would re-refresh (not applicable, blocked)
- [x] read-only children do not mutate — R1–R3 `capability: read_only`, `write_scope: []`
- [x] one writer owns accepted repairs — W1 admitted writer model per AGENT-ORCHESTRATION §4; zero repairs dispatched, so no race
- [x] at least one independent result materially verifies, refutes, or changes a claim — R2 refutes "green subset = ready" and verifies PR's DRAFT/NOT_PROVEN claim with primary artifact/CI evidence
- [x] raw logs/inventories remain outside the default root summary — see Overflow; only bounded identities and verdicts inlined
- [x] contradictory findings are resolved with evidence, not voting — R2 tension resolved via artifact provenance and `gh pr checks` primary evidence
- [x] a reviewer that edits the branch is reclassified as a fixer and the new head is reviewed afresh — documented as guardrail; not triggered
- [ ] the substantive PR merges and reconciles, **or** ends with a precise non-orchestration blocker — **blocked** (DRAFT + failing hosted check + NOT_PROVEN); recorded above with evidence, not an orchestration defect
- [x] the narrow control stays single-agent — this receipt, 1 writer
- [x] no portfolio database, scheduler, claim ledger, dashboard, branch-protection bypass, or product scope expansion — none built
- [x] orchestration defects become separate issues rather than contaminating the product PR — no orchestration defect filed in this pilot; would be filed separately if found

## Claim boundary

This receipt proves or refutes the existing-PR convergence pattern on one real PR per #1933. It does not establish universal productivity, nor does it require the same fan-out on future PRs. It makes no safety, UB-free, Miri-clean, site-execution, proof, or calibrated precision/recall claim. No event instrumentation or persistent orchestration database was built for this pilot.

## Repro

```bash
git fetch origin pull/2135/head
gh pr view 2135 --json baseRefOid,headRefOid,baseRefName,state,isDraft
gh pr checks 2135
git diff 4c77f3fc86b508d717722978eb8d622832b409e7..8d2ab49e8ab7f32ebdee46b4996d0d1d3d503751 --stat
cargo fmt --all -- --check
cargo run --locked -p xtask -- check-docs
```

Worktree for this pilot: `ur-lanes/pilot-1933` on branch `pilot/1933-converge` (parent `1e24e969`).
