# #2224 PR3 batch report: unfamiliar-batch seam counts
#
# Status: agent-drafted source-first batch. Draft evidence, not human
# adjudication, not calibration. All counts below are recomputed by
# `cargo run --locked -p xtask -- check-seam-batch` from `seams.toml`,
# `mapping.toml`, and the frozen outputs; the quoted totals match the
# mapping tally exactly.
#
# Analyzer: unsafe-review 0.3.8 at development revision
# `30c4b9eb1f290180ad8d6a46e31659f3373a9f88` (lane head at frozen-output
# run; skeleton base was origin/main `56366f4a`). One run per case with the
# manifest command shape against pinned detached head checkouts. The
# published-0.4.0 fixed-comparison slot is reserved and unfilled because
# 0.4.0 is not yet published.
#
# ## Raw counts (frozen inventory: 2 expected seams, 74 emitted cards)
#
# | Measure | Numerator | Denominator | Value |
# |---|---|---|---|
# | Seam recall (match / expected) | 1 | 2 | 1/2 |
# | Operation-family accuracy | 1 | 1 | 1/1 |
# | Obligation correctness (no over-credit, no missed guard/evidence) | 0 | 1 | 0/1 |
# | Obligation over-credit | 0 | 1 | 0/1 |
# | Reviewer-useful cards | 1 | 74 | 1/74 |
# | Duplicates | 0 | 74 | 0/74 |
# | Wrongly-surfaced cards | 73 | 74 | 73/74 |
# | Quiet-control cards (smallvec-847) | 0 | — | 0 |
#
# No rates are claimed. With 2 seams in 5 cases these are existence proofs
# and failure-shape notes, not precision or recall.
#
# ## Per-case slice
#
# | Case | Expected | Matched | Missing | Emitted | Useful |
# |---|---|---|---|---|---|
# | bytemuck-258 (dealloc UB fix) | 1 | 1 | 0 | 1 | 1 |
# | smallvec-847 (doc typo) | 0 | — | — | 0 | — |
# | bytemuck-349 (alignment contract docs + test) | 0 | — | — | 1 | 0 |
# | smallvec-757 (Arbitrary impl + tests) | 0 | — | — | 69 | 0 |
# | smallvec-768 (union repr + unsafe sig) | 1 | 0 | 1 | 3 | 0 |
#
# ## Per-family slice (canonical tool families, inventoried)
#
# | Family | Seams | Found | Obligation-correct |
# |---|---|---|---|
# | unsafe_fn_call | 1 | 1 | 0 |
# | unsafe_declaration | 1 | 0 | — |
#
# ## What the batch establishes
#
# - The tool flags the exact edited dealloc site in bytemuck-258 with the
#   right family and names callee-contract with the SAFETY comment present:
#   the UB-fix seam is found, not silent.
# - The designated quiet control (smallvec-847) draws zero cards: a second
#   clean quiet run after PR1's arrayvec-143 and PR2's bytes-744.
# - Zero-seam discipline holds without inflation: the three zero-seam
#   hardening/fixture cases contribute no seam rows, and their 70 cards are
#   counted wrongly-surfaced, not folded into recall.
#
# ## What the batch refutes or limits
#
# - Guard credit stays the weak axis. The 258-S1 card reports discharge
#   missing while the added dominating `size != 0` check establishes exactly
#   the ownership/allocation adequacy, and its next action would add a
#   redundant check: strictness is not understanding, now at 0/1 after PR1's
#   4/8 and PR2's 2/9.
# - Declaration-granularity recall misses. The 768-S1 signature/contract
#   change draws no unsafe_declaration card; the 3 emitted cards flag
#   pre-existing body operations (copy, add, drop_in_place) in the
#   surrounding safe functions whose safety arguments the repr retype does
#   not change. A changed unsafe declaration with a retained Safety section
#   is review-visible nowhere in the output: a miss shape distinct from the
#   guard-credit misses.
# - Context-line cards recur in stale-evidence form. The 349 card claims
#   contract_missing on an untouched internal call while the PR itself adds
#   the alignment-contract prose to the public wrapper: the card cannot see
#   the change it sits inside.
# - Fmt-reflow scope floods. smallvec-757's broad formatting reflow (patch
#   3/6 touches 101 lines across src/lib.rs) surfaces 69 pre-existing-site
#   cards, all wrongly-surfaced and none reviewer-useful: 0/69 useful on a
#   PR with no new or changed unsafe operation. Emission dominance without
#   seam dominance, after crossbeam-1297's 14/30 in PR2.
# - verify_commands name possibly-absent tests: spot check only, `miri test
#   from_buf` names a test with no counterpart in the head tree's tests/
#   (from_vec and drain do exist). The documented actions stay feasible;
#   the verification strings do not always verify.
#
# ## Partial runs and excluded inputs
#
# - smallvec-757: the frozen diff adds `tests/proptest.rs`, but the pinned
#   head tree no longer contains that file (the head commit "remove proptest
#   test file" deletes it), so the analyzer scanned only the resolved files
#   and warns accordingly. The unscanned file is safe-only test scaffolding
#   (proptest! cases calling `.len()`), so no seam is affected; no `unknown`
#   outcome is needed.
# - smallvec-768: the fetched patch series opens with a series base
#   (41f3ec5a) ahead of the recorded merge base (38a5afcb); the manifest
#   pins the PR's base/head SHAs and the full series bytes as fetched.
# - Cargo.lock churn (757) and safe-only fmt reflows (757, 768 doctest
#   patches) carry no seams and are excluded considerations, not silent
#   omissions. No inventory challenges: no flagged site changed semantics.
#
# ## Unresolved judgments and their effect
#
# - 768-S1 copy-line consideration: match-with-wrong-family rejected.
#   The from_buf copy_nonoverlapping card sits in an edited function, but
#   the flagged copy line itself is unchanged and its non-overlap/valid-range
#   argument is unchanged by the repr retype; the repr question belongs to
#   the declaration review. A match reading would give recall 2/2 with
#   family accuracy 1/2; the missing reading is recorded.
# - 258-S1 action direction: correct-but-generic chosen. The next action
#   (manual contract review plus guard evidence) is feasible, but the guard
#   evidence already exists one line up. An incorrect-action reading would
#   not move any count (no action-correctness tally exists); the note
#   carries the finding instead.
# - No vocabulary gaps were hit: both seam rows use only locked families
#   and keys (the 258-S1 dealloc site uses the nearest member unsafe_fn_call
#   per known-gap dealloc-family, recorded in the seam rationale).
#
# ## Uncertainty and limits
#
# - n=2 seams across 5 cases; correlated by project (3 smallvec, 2 bytemuck)
#   and by single agent drafter.
# - 69/74 emitted cards come from one fmt-reflow case; usefulness and
#   wrongly-surfaced counts are emission-dominated by that case.
# - Samples are unfamiliar (no overlap with PR1/PR2, pilots, exposed
#   development controls, or release-comparison inputs) but hand-picked, not
#   sampled: no population claim follows.
# - Empty denominators stay unknown: the zero-seam cases have no recall by
#   rule. The quiet control (smallvec-847) is clean at 0 cards; the other
#   two zero-seam cases are not quiet (1 and 69 cards, all wrongly-surfaced).
