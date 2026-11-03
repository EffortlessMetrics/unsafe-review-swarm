# #2224 PR3 batch report: unfamiliar-batch skeleton counts
#
# Status: agent-drafted source-first SKELETON. Draft structure, not human
# adjudication, not calibration. Frozen diffs and the source-first inventory
# are committed; frozen analyzer outputs are NOT yet acquired, so no recall
# or correctness counts exist. All counts below will be recomputed by
# `cargo run --locked -p xtask -- check-seam-batch` from `seams.toml`,
# `mapping.toml`, and the frozen outputs once the outputs lane lands; until
# then the checker fails closed on the missing outputs by design.
#
# Analyzer (reserved, no run yet): unsafe-review 0.3.8 at development
# revision `56366f4a43b7bbf338ef5e6c1ed842ae7c8ee116` (origin/main at batch
# assembly). The published-0.4.0 fixed-comparison slot is reserved and
# unfilled.
#
# ## Raw counts (frozen inventory: 2 expected seams, 0 emitted cards so far)
#
# | Measure | Numerator | Denominator | Value |
# |---|---|---|---|
# | Seam recall (match / expected) | unknown | 2 | unknown |
# | Operation-family accuracy | unknown | unknown | unknown |
# | Obligation correctness | unknown | unknown | unknown |
# | Obligation over-credit | unknown | unknown | unknown |
# | Reviewer-useful cards | unknown | unknown | unknown |
# | Duplicates | unknown | unknown | unknown |
# | Wrongly-surfaced cards | unknown | unknown | unknown |
# | Quiet-control cards (smallvec-847) | unknown | — | unknown |
#
# No rates are claimed. With 2 seams in 5 cases the eventual numbers will be
# existence proofs and failure-shape notes, not precision or recall.
#
# ## Per-case slice (inventory only)
#
# | Case | Expected | Role |
# |---|---|---|
# | bytemuck-258 (dealloc UB fix) | 1 | unsafe-relevant |
# | smallvec-847 (doc typo) | 0 | quiet control |
# | bytemuck-349 (alignment contract docs + test) | 0 | hardening, no new unsafe op |
# | smallvec-757 (Arbitrary impl + tests) | 0 | test/fixture-heavy, no new unsafe op |
# | smallvec-768 (union repr + unsafe sig) | 1 | non-NonNull |
#
# ## Per-family slice (canonical tool families, inventoried)
#
# | Family | Seams |
# |---|---|
# | unsafe_fn_call | 1 |
# | unsafe_declaration | 1 |
#
# ## What remains
#
# - Acquire pinned head checkouts, run the analyzer once per case with the
#   recorded command shape, and commit frozen outputs under `outputs/` with
#   sha256 pins (the outputs lane; no tuning, no reruns as new evidence).
# - Replace the two `unknown` mapping rows with match/missing judgments
#   answering all four baseline questions, add one row per emitted card, and
#   fill the tally; keep unresolved judgments and vocabulary gaps visible.
# - Confirm the smallvec-768-S1 debug-only guard judgment and the
#   bytemuck-258-S1 stale-evidence direction in mapping notes.
#
# ## Inventory notes (not findings)
#
# - Three zero-seam cases is honest per the safe-API-test precedent, but it
#   concentrates the recall denominator on two seams; the outputs lane must
#   not inflate the denominator with safe-call rows to compensate.
# - smallvec-757's 57KB diff is dominated by Cargo.lock churn and a safe-only
#   fmt reflow; both are excluded considerations, not seams.
# - smallvec-768's fetched patch opens with a series base (41f3ec5a) ahead of
#   the recorded merge base (38a5afcb); the manifest pins the PR's base/head
#   SHAs and the full base...head diff bytes as fetched.
# - Two projects cover five cases (smallvec x3, bytemuck x2): fresh repos with
#   small hand-labelable diffs, but correlated by project; noted, not fixed.
#
# ## Unresolved judgments and their effect
#
# - None yet: mapping is all-unknown. The debug-only guard question on
#   768-S1 and the stale-comment-evidence direction on 258-S1 are flagged in
#   seam rationales for the mapping pass, with no count effect until then.
#
# ## Uncertainty and limits
#
# - n=2 seams across 5 cases; correlated by project (3 smallvec, 2 bytemuck)
#   and by single agent drafter.
# - Base SHAs verified to exist via the forge commits API and head SHAs via
#   pull-ref ls-remote; no head checkouts were made, so enclosing-context
#   reads came from pinned file fetches, not a full tree.
# - Unsupported families, parse failures, and partial runs: none inventoried;
#   the evaluator supports `unknown` outcomes and the PR1 tests probe them.
# - Empty denominators stay unknown: the zero-seam cases have no recall.
